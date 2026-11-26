use crate::{
    AdBeaconSink, AdCache, AdError, AdPayload, AdSource, CachedAds, Clock, RawAd, WebPageOpener,
};
use motogp_domain::{AdCreative, AdSlot};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// How long a fetched feed is served without asking the source again.
pub const DEFAULT_AD_TTL: Duration = Duration::from_secs(6 * 60 * 60);

/// What the UI receives for a slot. It carries neither the click URL nor the
/// impression URL: the UI only ever hands back the creative id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdView {
    pub slot: AdSlot,
    pub creative_id: String,
    pub title: String,
    pub body: String,
    pub image_data_uri: Option<String>,
    pub sponsor_label: String,
}

/// Validates a downloaded feed: unparseable, invalid and expired creatives are
/// dropped, and the first creative of a duplicated id wins.
pub fn validate_ads(payload: AdPayload, now_ms: u64) -> Vec<AdCreative> {
    let mut seen = HashSet::new();
    let mut creatives = Vec::new();
    for raw in payload.creatives {
        let RawAd::Draft(draft) = raw else { continue };
        let Ok(creative) = AdCreative::from_draft(draft) else {
            continue;
        };
        if creative.is_expired(now_ms) || !seen.insert(creative.id().to_string()) {
            continue;
        }
        creatives.push(creative);
    }
    creatives
}

#[derive(Default)]
struct State {
    /// `None` until the first call that needs the creatives.
    creatives: Option<Vec<AdCreative>>,
    /// When the feed was last attempted (success or failure), or the fetch
    /// time of the fresh cache that was used instead. The next fetch happens
    /// once a TTL has passed since then.
    last_attempt_ms: u64,
    /// Next candidate index per slot (round-robin).
    cursors: HashMap<AdSlot, usize>,
    /// Creative ids whose impression was already counted this session.
    beaconed: HashSet<String>,
}

/// Use case: picks a sponsored creative per slot from the self-hosted feed,
/// counts impressions once per session and opens click URLs by creative id.
pub struct AdService<S, C, B, K> {
    source: S,
    cache: C,
    beacon: B,
    clock: K,
    ttl_ms: u64,
    state: Mutex<State>,
}

impl<S: AdSource, C: AdCache, B: AdBeaconSink, K: Clock> AdService<S, C, B, K> {
    /// Service with the default 6 hour TTL.
    pub fn new(source: S, cache: C, beacon: B, clock: K) -> Self {
        Self::with_ttl(source, cache, beacon, clock, DEFAULT_AD_TTL)
    }

    pub fn with_ttl(source: S, cache: C, beacon: B, clock: K, ttl: Duration) -> Self {
        Self {
            source,
            cache,
            beacon,
            clock,
            ttl_ms: u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX),
            state: Mutex::new(State::default()),
        }
    }

    /// The next creative of `slot`, or `None` when ads are off or nothing can
    /// be shown. `ads_enabled = false` touches no port. Feed problems are never
    /// errors.
    pub fn get_ad(&self, slot: AdSlot, ads_enabled: bool) -> Result<Option<AdView>, AdError> {
        if !ads_enabled {
            return Ok(None);
        }
        let now = self.clock.now_millis();
        let mut state = self.lock();
        let candidates = self.candidates(&mut state, slot, now);
        if candidates.is_empty() {
            return Ok(None);
        }
        let cursor = state.cursors.entry(slot).or_insert(0);
        let creative = &candidates[*cursor % candidates.len()];
        *cursor = (*cursor + 1) % candidates.len();
        Ok(Some(AdView {
            slot,
            creative_id: creative.id().to_string(),
            title: creative.title().to_string(),
            body: creative.body().to_string(),
            image_data_uri: creative.image_data_uri().map(String::from),
            sponsor_label: creative.sponsor_label().to_string(),
        }))
    }

    /// Called by the UI after the slot really rendered. Sends the impression
    /// beacon once per creative per session, only when the creative has an
    /// impression URL. A beacon failure is swallowed.
    pub fn report_shown(&self, slot: AdSlot, creative_id: &str) -> Result<(), AdError> {
        let now = self.clock.now_millis();
        let mut state = self.lock();
        let creative = self.find(&mut state, slot, creative_id, now)?;
        let Some(url) = creative.impression_url() else {
            return Ok(());
        };
        if !state.beaconed.insert(creative.id().to_string()) {
            return Ok(());
        }
        // Sponsors count on their side; a failed beacon must never reach the user.
        let _ = self.beacon.send_impression(url);
        Ok(())
    }

    /// Opens the click URL of the creative `creative_id` of `slot`. The caller
    /// supplies only the id, never a URL.
    pub fn open_click(
        &self,
        slot: AdSlot,
        creative_id: &str,
        opener: &dyn WebPageOpener,
    ) -> Result<(), AdError> {
        let now = self.clock.now_millis();
        let url = {
            let mut state = self.lock();
            self.find(&mut state, slot, creative_id, now)?
                .click_url()
                .to_string()
        };
        opener.open_page(&url).map_err(AdError::Open)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Creatives of `slot` that have not expired, in feed order.
    fn candidates(&self, state: &mut State, slot: AdSlot, now: u64) -> Vec<AdCreative> {
        self.creatives(state, now)
            .iter()
            .filter(|c| c.slot() == slot && !c.is_expired(now))
            .cloned()
            .collect()
    }

    fn find(
        &self,
        state: &mut State,
        slot: AdSlot,
        id: &str,
        now: u64,
    ) -> Result<AdCreative, AdError> {
        self.creatives(state, now)
            .iter()
            .find(|c| c.slot() == slot && c.id() == id && !c.is_expired(now))
            .cloned()
            .ok_or_else(|| AdError::UnknownCreative {
                slot,
                id: id.to_string(),
            })
    }

    /// The memoized creatives. Loaded on first use (a fresh cache wins,
    /// otherwise the source; when the source fails, any cached copy). Once a
    /// TTL has passed since the last attempt the source is asked again: a
    /// success replaces the set, a failure keeps the current one. Rotation
    /// cursors and impression dedupe live apart from the set and survive.
    fn creatives<'a>(&self, state: &'a mut State, now: u64) -> &'a [AdCreative] {
        if state.creatives.is_none() {
            let (creatives, attempted_at) = self.load(now);
            state.creatives = Some(creatives);
            state.last_attempt_ms = attempted_at;
        }
        if now.saturating_sub(state.last_attempt_ms) >= self.ttl_ms {
            state.last_attempt_ms = now;
            match self.source.fetch() {
                Ok(payload) => {
                    let creatives = validate_ads(payload, now);
                    self.save(now, &creatives);
                    state.creatives = Some(creatives);
                }
                Err(_) => {
                    if state.creatives.as_ref().is_some_and(Vec::is_empty) {
                        if let Some(cached) = self.cache.load().ok().flatten() {
                            state.creatives = Some(cached.creatives);
                        }
                    }
                }
            }
        }
        state.creatives.as_deref().unwrap_or_default()
    }

    /// First load: returns the creatives and the time of the attempt they
    /// stem from (the cache's fetch time when the cache was fresh).
    fn load(&self, now: u64) -> (Vec<AdCreative>, u64) {
        let cached = self.cache.load().ok().flatten();
        if let Some(cached) = &cached {
            if now.saturating_sub(cached.fetched_at_ms) < self.ttl_ms {
                return (cached.creatives.clone(), cached.fetched_at_ms);
            }
        }
        match self.source.fetch() {
            Ok(payload) => {
                let creatives = validate_ads(payload, now);
                self.save(now, &creatives);
                (creatives, now)
            }
            Err(_) => (cached.map(|c| c.creatives).unwrap_or_default(), now),
        }
    }

    fn save(&self, now: u64, creatives: &[AdCreative]) {
        // The cache is an optimisation: a failed save changes nothing.
        let _ = self.cache.save(&CachedAds {
            fetched_at_ms: now,
            creatives: creatives.to_vec(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdBeaconError, AdCacheError, AdSourceError, AppError};
    use motogp_domain::AdCreativeDraft;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;

    const NOW: u64 = 1_800_000_000_000;
    const HOUR: u64 = 3_600_000;

    #[derive(Clone)]
    struct Clk(Arc<AtomicU64>);
    impl Clock for Clk {
        fn now_millis(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    #[derive(Clone)]
    struct Src {
        answer: Arc<Mutex<Result<AdPayload, AdSourceError>>>,
        calls: Arc<AtomicUsize>,
    }
    impl AdSource for Src {
        fn fetch(&self) -> Result<AdPayload, AdSourceError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.answer.lock().unwrap().clone()
        }
    }

    #[derive(Clone, Default)]
    struct Cache {
        stored: Arc<Mutex<Option<CachedAds>>>,
        load_fails: bool,
        save_fails: bool,
        saves: Arc<AtomicUsize>,
    }
    impl AdCache for Cache {
        fn load(&self) -> Result<Option<CachedAds>, AdCacheError> {
            if self.load_fails {
                return Err(AdCacheError("corrupt".into()));
            }
            Ok(self.stored.lock().unwrap().clone())
        }
        fn save(&self, ads: &CachedAds) -> Result<(), AdCacheError> {
            if self.save_fails {
                return Err(AdCacheError("disk full".into()));
            }
            self.saves.fetch_add(1, Ordering::SeqCst);
            *self.stored.lock().unwrap() = Some(ads.clone());
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct Beacon {
        sent: Arc<Mutex<Vec<String>>>,
        fails: bool,
    }
    impl AdBeaconSink for Beacon {
        fn send_impression(&self, url: &str) -> Result<(), AdBeaconError> {
            self.sent.lock().unwrap().push(url.to_string());
            if self.fails {
                return Err(AdBeaconError("down".into()));
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct Opener {
        opened: Mutex<Vec<String>>,
        fails: bool,
    }
    impl WebPageOpener for Opener {
        fn open_page(&self, url: &str) -> Result<(), AppError> {
            if self.fails {
                return Err(AppError::Read {
                    location: url.into(),
                    detail: "no browser".into(),
                });
            }
            self.opened.lock().unwrap().push(url.to_string());
            Ok(())
        }
    }

    fn draft(id: &str, slot: &str) -> AdCreativeDraft {
        AdCreativeDraft {
            id: id.into(),
            slot: slot.into(),
            title: format!("Title {id}"),
            body: "Body".into(),
            image_data_uri: Some("data:image/png;base64,AAAA".into()),
            click_url: format!("https://example.com/{id}"),
            sponsor_label: "Acme".into(),
            impression_url: Some(format!("https://example.com/i/{id}")),
            expires_at_ms: None,
        }
    }

    fn payload(drafts: Vec<AdCreativeDraft>) -> AdPayload {
        AdPayload {
            creatives: drafts.into_iter().map(RawAd::Draft).collect(),
        }
    }

    struct Rig {
        src: Src,
        cache: Cache,
        beacon: Beacon,
        clock: Clk,
    }

    impl Rig {
        fn serving(drafts: Vec<AdCreativeDraft>) -> Self {
            Self::answering(Ok(payload(drafts)))
        }
        fn answering(answer: Result<AdPayload, AdSourceError>) -> Self {
            Rig {
                src: Src {
                    answer: Arc::new(Mutex::new(answer)),
                    calls: Arc::default(),
                },
                cache: Cache::default(),
                beacon: Beacon::default(),
                clock: Clk(Arc::new(AtomicU64::new(NOW))),
            }
        }
        fn service(&self) -> AdService<Src, Cache, Beacon, Clk> {
            AdService::new(
                self.src.clone(),
                self.cache.clone(),
                self.beacon.clone(),
                self.clock.clone(),
            )
        }
        fn fetches(&self) -> usize {
            self.src.calls.load(Ordering::SeqCst)
        }
        fn beacons(&self) -> Vec<String> {
            self.beacon.sent.lock().unwrap().clone()
        }
    }

    fn cached(fetched_at_ms: u64, drafts: Vec<AdCreativeDraft>) -> CachedAds {
        CachedAds {
            fetched_at_ms,
            creatives: validate_ads(payload(drafts), 0),
        }
    }

    #[test]
    fn validate_ads_drops_invalid_expired_unparseable_and_duplicates() {
        let mut http = draft("http", "discover");
        http.click_url = "http://example.com".into();
        let mut expired = draft("old", "discover");
        expired.expires_at_ms = Some(NOW);
        let mut duplicate = draft("dup", "sidebar");
        duplicate.title = "Second".into();
        let mut live = draft("live", "about");
        live.expires_at_ms = Some(NOW + 1);
        let raw = AdPayload {
            creatives: vec![
                RawAd::Draft(draft("dup", "discover")),
                RawAd::Draft(http),
                RawAd::Draft(expired),
                RawAd::Unparseable {
                    reason: "bad".into(),
                },
                RawAd::Draft(duplicate),
                RawAd::Draft(live),
            ],
        };
        let ids: Vec<String> = validate_ads(raw, NOW)
            .iter()
            .map(|c| c.id().to_string())
            .collect();
        assert_eq!(ids, ["dup", "live"]);
    }

    #[test]
    fn the_view_carries_no_urls_and_fetches_then_caches() {
        let rig = Rig::serving(vec![draft("a", "discover")]);
        let view = rig
            .service()
            .get_ad(AdSlot::Discover, true)
            .unwrap()
            .unwrap();
        assert_eq!(
            view,
            AdView {
                slot: AdSlot::Discover,
                creative_id: "a".into(),
                title: "Title a".into(),
                body: "Body".into(),
                image_data_uri: Some("data:image/png;base64,AAAA".into()),
                sponsor_label: "Acme".into(),
            }
        );
        let saved = rig.cache.stored.lock().unwrap().clone().unwrap();
        assert_eq!((saved.fetched_at_ms, saved.creatives.len()), (NOW, 1));
    }

    #[test]
    fn disabled_ads_touch_no_port() {
        let rig = Rig::serving(vec![draft("a", "discover")]);
        let svc = rig.service();
        assert_eq!(svc.get_ad(AdSlot::Discover, false), Ok(None));
        assert_eq!(rig.fetches(), 0);
        assert_eq!(rig.cache.saves.load(Ordering::SeqCst), 0);
        assert!(rig.beacons().is_empty());
    }

    #[test]
    fn a_fresh_cache_is_used_without_fetching() {
        let rig = Rig::serving(vec![draft("remote", "discover")]);
        *rig.cache.stored.lock().unwrap() =
            Some(cached(NOW - HOUR, vec![draft("cached", "discover")]));
        let view = rig
            .service()
            .get_ad(AdSlot::Discover, true)
            .unwrap()
            .unwrap();
        assert_eq!(view.creative_id, "cached");
        assert_eq!(rig.fetches(), 0);
    }

    #[test]
    fn a_stale_cache_is_refetched() {
        let rig = Rig::serving(vec![draft("remote", "discover")]);
        *rig.cache.stored.lock().unwrap() =
            Some(cached(NOW - 7 * HOUR, vec![draft("cached", "discover")]));
        let view = rig
            .service()
            .get_ad(AdSlot::Discover, true)
            .unwrap()
            .unwrap();
        assert_eq!(view.creative_id, "remote");
        assert_eq!(rig.fetches(), 1);
    }

    #[test]
    fn a_custom_ttl_decides_freshness() {
        let rig = Rig::serving(vec![draft("remote", "discover")]);
        *rig.cache.stored.lock().unwrap() =
            Some(cached(NOW - HOUR, vec![draft("cached", "discover")]));
        let svc = AdService::with_ttl(
            rig.src.clone(),
            rig.cache.clone(),
            rig.beacon.clone(),
            rig.clock.clone(),
            Duration::from_secs(60),
        );
        let view = svc.get_ad(AdSlot::Discover, true).unwrap().unwrap();
        assert_eq!(view.creative_id, "remote");
    }

    #[test]
    fn a_failed_fetch_falls_back_to_an_old_cache() {
        let rig = Rig::answering(Err(AdSourceError::Network("down".into())));
        *rig.cache.stored.lock().unwrap() =
            Some(cached(NOW - 100 * HOUR, vec![draft("cached", "discover")]));
        let view = rig
            .service()
            .get_ad(AdSlot::Discover, true)
            .unwrap()
            .unwrap();
        assert_eq!(view.creative_id, "cached");
        assert_eq!(rig.fetches(), 1);
    }

    #[test]
    fn an_unreachable_feed_with_no_cache_is_not_an_error() {
        for failure in [
            AdSourceError::NotPublished,
            AdSourceError::Network("down".into()),
            AdSourceError::Http { status: 500 },
            AdSourceError::TooLarge { limit: 1 },
            AdSourceError::Invalid("junk".into()),
        ] {
            let rig = Rig::answering(Err(failure));
            assert_eq!(rig.service().get_ad(AdSlot::Discover, true), Ok(None));
        }
    }

    #[test]
    fn a_failed_attempt_is_not_repeated_on_every_call() {
        let rig = Rig::answering(Err(AdSourceError::Network("down".into())));
        let svc = rig.service();
        for _ in 0..3 {
            assert_eq!(svc.get_ad(AdSlot::Discover, true), Ok(None));
        }
        assert_eq!(rig.fetches(), 1);
    }

    fn advance(rig: &Rig, ms: u64) {
        rig.clock.0.fetch_add(ms, Ordering::SeqCst);
    }

    fn shown_id(svc: &AdService<Src, Cache, Beacon, Clk>) -> String {
        svc.get_ad(AdSlot::Discover, true)
            .unwrap()
            .unwrap()
            .creative_id
    }

    #[test]
    fn a_failed_fetch_is_retried_after_the_ttl() {
        let rig = Rig::answering(Err(AdSourceError::Network("down".into())));
        let svc = rig.service();
        assert_eq!(svc.get_ad(AdSlot::Discover, true), Ok(None));
        *rig.src.answer.lock().unwrap() = Ok(payload(vec![draft("late", "discover")]));
        advance(&rig, 5 * HOUR);
        assert_eq!(svc.get_ad(AdSlot::Discover, true), Ok(None));
        assert_eq!(rig.fetches(), 1);
        advance(&rig, 2 * HOUR);
        assert_eq!(shown_id(&svc), "late");
        assert_eq!(rig.fetches(), 2);
    }

    #[test]
    fn a_successful_fetch_is_refreshed_after_the_ttl() {
        let rig = Rig::serving(vec![draft("old", "discover")]);
        let svc = rig.service();
        assert_eq!(shown_id(&svc), "old");
        *rig.src.answer.lock().unwrap() = Ok(payload(vec![draft("new", "discover")]));
        advance(&rig, 7 * HOUR);
        assert_eq!(shown_id(&svc), "new");
        assert_eq!(rig.fetches(), 2);
        let saved = rig.cache.stored.lock().unwrap().clone().unwrap();
        assert_eq!(saved.fetched_at_ms, NOW + 7 * HOUR);
        // No further fetch within the new window.
        shown_id(&svc);
        assert_eq!(rig.fetches(), 2);
    }

    #[test]
    fn a_failed_refresh_keeps_serving_the_previous_set() {
        let rig = Rig::serving(vec![draft("old", "discover")]);
        let svc = rig.service();
        shown_id(&svc);
        *rig.src.answer.lock().unwrap() = Err(AdSourceError::Network("down".into()));
        advance(&rig, 7 * HOUR);
        assert_eq!(shown_id(&svc), "old");
        assert_eq!(rig.fetches(), 2);
        // The failed attempt is not repeated before another TTL.
        shown_id(&svc);
        assert_eq!(rig.fetches(), 2);
    }

    #[test]
    fn a_refresh_keeps_the_rotation_and_the_beacon_dedupe() {
        let rig = Rig::serving(vec![draft("a", "discover"), draft("b", "discover")]);
        let svc = rig.service();
        assert_eq!(shown_id(&svc), "a");
        svc.report_shown(AdSlot::Discover, "a").unwrap();
        advance(&rig, 7 * HOUR);
        assert_eq!(shown_id(&svc), "b");
        assert_eq!(rig.fetches(), 2);
        svc.report_shown(AdSlot::Discover, "a").unwrap();
        assert_eq!(rig.beacons(), ["https://example.com/i/a"]);
    }

    #[test]
    fn cache_load_and_save_errors_are_ignored() {
        let mut rig = Rig::serving(vec![draft("a", "discover")]);
        rig.cache.load_fails = true;
        rig.cache.save_fails = true;
        let view = rig.service().get_ad(AdSlot::Discover, true).unwrap();
        assert_eq!(view.unwrap().creative_id, "a");
    }

    #[test]
    fn expired_creatives_are_never_shown_even_after_loading() {
        let mut expiring = draft("a", "sidebar");
        expiring.expires_at_ms = Some(NOW + HOUR);
        let rig = Rig::serving(vec![expiring]);
        let svc = rig.service();
        assert!(svc.get_ad(AdSlot::Sidebar, true).unwrap().is_some());
        rig.clock.0.store(NOW + HOUR, Ordering::SeqCst);
        assert_eq!(svc.get_ad(AdSlot::Sidebar, true), Ok(None));
        assert!(matches!(
            svc.report_shown(AdSlot::Sidebar, "a"),
            Err(AdError::UnknownCreative { .. })
        ));
    }

    #[test]
    fn only_creatives_of_the_requested_slot_are_offered() {
        let rig = Rig::serving(vec![draft("a", "discover")]);
        assert_eq!(rig.service().get_ad(AdSlot::About, true), Ok(None));
    }

    #[test]
    fn creatives_of_a_slot_rotate_in_feed_order() {
        let rig = Rig::serving(vec![
            draft("a", "discover"),
            draft("x", "about"),
            draft("b", "discover"),
        ]);
        let svc = rig.service();
        let ids: Vec<String> = (0..5)
            .map(|_| {
                svc.get_ad(AdSlot::Discover, true)
                    .unwrap()
                    .unwrap()
                    .creative_id
            })
            .collect();
        assert_eq!(ids, ["a", "b", "a", "b", "a"]);
        // Another slot has its own cursor.
        assert_eq!(
            svc.get_ad(AdSlot::About, true)
                .unwrap()
                .unwrap()
                .creative_id,
            "x"
        );
        assert_eq!(rig.fetches(), 1);
    }

    #[test]
    fn the_impression_beacon_is_sent_once_per_creative() {
        let rig = Rig::serving(vec![draft("a", "discover"), draft("b", "discover")]);
        let svc = rig.service();
        svc.report_shown(AdSlot::Discover, "a").unwrap();
        svc.report_shown(AdSlot::Discover, "a").unwrap();
        svc.report_shown(AdSlot::Discover, "b").unwrap();
        assert_eq!(
            rig.beacons(),
            ["https://example.com/i/a", "https://example.com/i/b"]
        );
    }

    #[test]
    fn a_creative_without_impression_url_sends_no_beacon() {
        let mut house = draft("house", "about");
        house.impression_url = None;
        let rig = Rig::serving(vec![house]);
        rig.service().report_shown(AdSlot::About, "house").unwrap();
        assert!(rig.beacons().is_empty());
    }

    #[test]
    fn a_failing_beacon_is_swallowed_and_not_retried() {
        let mut rig = Rig::serving(vec![draft("a", "discover")]);
        rig.beacon.fails = true;
        let svc = rig.service();
        assert_eq!(svc.report_shown(AdSlot::Discover, "a"), Ok(()));
        assert_eq!(svc.report_shown(AdSlot::Discover, "a"), Ok(()));
        assert_eq!(rig.beacons().len(), 1);
    }

    #[test]
    fn reporting_an_unknown_creative_or_wrong_slot_is_an_error() {
        let rig = Rig::serving(vec![draft("a", "discover")]);
        let svc = rig.service();
        for (slot, id) in [(AdSlot::Discover, "zzz"), (AdSlot::About, "a")] {
            assert_eq!(
                svc.report_shown(slot, id),
                Err(AdError::UnknownCreative {
                    slot,
                    id: id.to_string()
                })
            );
        }
        assert!(rig.beacons().is_empty());
    }

    #[test]
    fn clicking_opens_the_click_url_of_the_creative() {
        let rig = Rig::serving(vec![draft("a", "discover")]);
        let opener = Opener::default();
        rig.service()
            .open_click(AdSlot::Discover, "a", &opener)
            .unwrap();
        assert_eq!(*opener.opened.lock().unwrap(), ["https://example.com/a"]);
    }

    #[test]
    fn clicking_an_unknown_creative_opens_nothing() {
        let rig = Rig::serving(vec![draft("a", "discover")]);
        let opener = Opener::default();
        let result = rig
            .service()
            .open_click(AdSlot::Discover, "not-offered", &opener);
        assert_eq!(
            result,
            Err(AdError::UnknownCreative {
                slot: AdSlot::Discover,
                id: "not-offered".into()
            })
        );
        assert!(opener.opened.lock().unwrap().is_empty());
    }

    #[test]
    fn an_opener_failure_is_reported_as_open() {
        let rig = Rig::serving(vec![draft("a", "discover")]);
        let opener = Opener {
            fails: true,
            ..Opener::default()
        };
        let result = rig.service().open_click(AdSlot::Discover, "a", &opener);
        assert!(matches!(result, Err(AdError::Open(AppError::Read { .. }))));
    }
}
