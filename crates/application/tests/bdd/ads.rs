//! Steps for `tests/features/ads.feature` (issue #101), with in-memory fakes of
//! the ad ports. Traceability: docs/validation-workflow.md.
use crate::ad_fakes::*;
use crate::catalog_fakes::{FakeClock, FakeWebOpener, NOW_MS};
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{validate_ads, AdError, AdService, AdSourceError, AdView};
use motogp_domain::{AdCreative, AdSlot};

#[derive(World)]
struct AdsWorld {
    source: FakeAdSource,
    cache: FakeAdCache,
    beacon: FakeBeacon,
    clock: FakeClock,
    opener: FakeWebOpener,
    enabled: bool,
    /// Kept between steps: rotation and beacon dedupe live inside the service.
    svc: Option<TestAdService>,
    views: Vec<Option<AdView>>,
    creatives: Vec<AdCreative>,
    click: Option<Result<(), AdError>>,
}

impl Default for AdsWorld {
    fn default() -> Self {
        Self {
            source: FakeAdSource::default(),
            cache: FakeAdCache::default(),
            beacon: FakeBeacon::default(),
            clock: FakeClock::at(NOW_MS),
            opener: FakeWebOpener::default(),
            enabled: false,
            svc: None,
            views: Vec::new(),
            creatives: Vec::new(),
            click: None,
        }
    }
}

impl std::fmt::Debug for AdsWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdsWorld")
    }
}

impl AdsWorld {
    fn svc(&mut self) -> &TestAdService {
        let (source, cache, beacon, clock) = (
            self.source.clone(),
            self.cache.clone(),
            self.beacon.clone(),
            self.clock.clone(),
        );
        self.svc
            .get_or_insert_with(|| AdService::new(source, cache, beacon, clock))
    }
    fn ask(&mut self, slot: AdSlot) {
        let enabled = self.enabled;
        let view = self
            .svc()
            .get_ad(slot, enabled, None)
            .expect("get_ad never fails");
        self.views.push(view);
    }
}

fn slot(name: &str) -> AdSlot {
    AdSlot::parse(name).unwrap_or_else(|| panic!("unknown slot {name}"))
}

#[given(expr = "the feed has a creative for the {string} slot")]
fn one_creative(w: &mut AdsWorld, slot: String) {
    w.source = FakeAdSource::serving(vec![creative_draft("acme-1", &slot)]);
}

#[given("ads are enabled")]
fn enabled(w: &mut AdsWorld) {
    w.enabled = true;
}

#[given("the user turned ads off")]
fn disabled(w: &mut AdsWorld) {
    w.enabled = false;
}

#[given(expr = "the feed has a creative whose click URL uses {string}")]
fn insecure(w: &mut AdsWorld, scheme: String) {
    let mut draft = creative_draft("acme-1", "discover");
    draft.click_url = format!("{scheme}://sponsor.example.com/click");
    w.source = FakeAdSource::serving(vec![draft]);
}

#[given(expr = "the feed has only an expired creative for the {string} slot")]
fn expired(w: &mut AdsWorld, slot: String) {
    let mut draft = creative_draft("old-1", &slot);
    draft.expires_at_ms = Some(NOW_MS - 1);
    w.source = FakeAdSource::serving(vec![draft]);
}

#[given("the feed is unreachable and the cache is empty")]
fn unreachable(w: &mut AdsWorld) {
    w.source = FakeAdSource::failing(AdSourceError::Network("offline".into()));
}

#[given(expr = "the feed has two creatives for the {string} slot")]
fn two_creatives(w: &mut AdsWorld, slot: String) {
    w.source = FakeAdSource::serving(vec![
        creative_draft("acme-1", &slot),
        creative_draft("acme-2", &slot),
    ]);
}

#[given(expr = "the feed has a creative with an impression URL for the {string} slot")]
fn with_impression(w: &mut AdsWorld, slot: String) {
    w.source = FakeAdSource::serving(vec![creative_draft("acme-1", &slot)]);
}

#[given(expr = "the feed has a creative without an impression URL for the {string} slot")]
fn house_ad(w: &mut AdsWorld, slot: String) {
    let mut draft = creative_draft("house-1", &slot);
    draft.impression_url = None;
    w.source = FakeAdSource::serving(vec![draft]);
}

#[when(expr = "the UI asks for the {string} ad")]
fn ask_one(w: &mut AdsWorld, name: String) {
    w.ask(slot(&name));
}

#[when(expr = "the UI asks for the {string} ad twice")]
fn ask_twice(w: &mut AdsWorld, name: String) {
    w.ask(slot(&name));
    w.ask(slot(&name));
}

#[when("the UI asks for any ad")]
fn ask_any(w: &mut AdsWorld) {
    w.ask(AdSlot::Discover);
}

#[when("the feed is read")]
fn read_feed(w: &mut AdsWorld) {
    w.creatives = validate_ads(w.source.payload(), NOW_MS);
}

/// The id of the creative the fixture serves for `slot`.
fn shown_id(w: &mut AdsWorld, name: &str) -> String {
    w.enabled = true;
    w.ask(slot(name));
    w.views
        .last()
        .cloned()
        .flatten()
        .expect("the slot has a creative")
        .creative_id
}

#[when(expr = "the UI reports the {string} ad as shown twice")]
fn shown_twice(w: &mut AdsWorld, name: String) {
    let id = shown_id(w, &name);
    for _ in 0..2 {
        w.svc().report_shown(slot(&name), &id).unwrap();
    }
}

#[when(expr = "the UI reports the {string} ad as shown")]
fn shown_once(w: &mut AdsWorld, name: String) {
    let id = shown_id(w, &name);
    w.svc().report_shown(slot(&name), &id).unwrap();
}

#[when(expr = "the UI clicks the {string} ad")]
fn click(w: &mut AdsWorld, name: String) {
    let id = shown_id(w, &name);
    click_id(w, &name, &id);
}

#[when(expr = "the UI clicks the {string} ad with the id {string}")]
fn click_unknown(w: &mut AdsWorld, name: String, id: String) {
    click_id(w, &name, &id);
}

fn click_id(w: &mut AdsWorld, name: &str, id: &str) {
    let opener = w.opener.clone();
    w.click = Some(w.svc().open_click(slot(name), id, &opener));
}

#[then(expr = "it receives the title, body, image and a {string} label")]
fn receives(w: &mut AdsWorld, _label: String) {
    let view = w.views[0].as_ref().expect("a creative was returned");
    assert_eq!(view.title, "Title of acme-1");
    assert_eq!(view.body, "Grip for every lap");
    assert!(view.image_data_uri.is_some());
    // The UI prefixes the visible "Sponsored" label; the view carries the sponsor name.
    assert_eq!(view.sponsor_label, SPONSOR_LABEL);
}

#[then("it does not receive the click URL")]
fn no_click_url(w: &mut AdsWorld) {
    let shown = format!("{:?}", w.views[0]);
    assert!(!shown.contains(&click_url("acme-1")), "{shown}");
    assert!(!shown.contains(&impression_url("acme-1")), "{shown}");
}

#[then("no creative is returned and nothing is fetched")]
fn nothing_fetched(w: &mut AdsWorld) {
    assert_eq!(w.views, vec![None]);
    assert_eq!(w.source.calls(), 0);
    assert!(w.cache.stored().is_none());
    assert!(w.beacon.sent().is_empty());
}

#[then("that creative is dropped")]
fn dropped(w: &mut AdsWorld) {
    assert!(w.creatives.is_empty());
}

#[then("no creative is returned")]
fn none_returned(w: &mut AdsWorld) {
    assert_eq!(w.views, vec![None]);
}

#[then("no creative is returned and no error is shown")]
fn none_and_no_error(w: &mut AdsWorld) {
    // `ask` already unwrapped the Result: reaching here means no error surfaced.
    assert_eq!(w.views, vec![None]);
}

#[then("it receives each creative once")]
fn each_once(w: &mut AdsWorld) {
    let ids: Vec<&str> = w
        .views
        .iter()
        .map(|v| v.as_ref().expect("a creative").creative_id.as_str())
        .collect();
    assert_eq!(ids, ["acme-1", "acme-2"]);
}

#[then("exactly one impression beacon is sent")]
fn one_beacon(w: &mut AdsWorld) {
    assert_eq!(w.beacon.sent(), vec![impression_url("acme-1")]);
}

#[then("no impression beacon is sent")]
fn no_beacon(w: &mut AdsWorld) {
    assert!(w.beacon.sent().is_empty());
}

#[then("the creative's click URL is opened")]
fn click_opened(w: &mut AdsWorld) {
    assert_eq!(w.click, Some(Ok(())));
    assert_eq!(w.opener.opened(), vec![click_url("acme-1")]);
}

#[then("nothing is opened and an unknown creative error is returned")]
fn unknown_creative(w: &mut AdsWorld) {
    assert_eq!(
        w.click,
        Some(Err(AdError::UnknownCreative {
            slot: AdSlot::Discover,
            id: "not-offered".into()
        }))
    );
    assert!(w.opener.opened().is_empty());
}

/// Runs only `ads.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = AdsWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("ads.feature"))
        .await;
    !writer.execution_has_failed()
}
