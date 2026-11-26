//! HTTPS adapters of the sponsored ads (issue #102): [`HttpAdSource`] downloads
//! the self-hosted `ads.json` feed and the creative images, and
//! [`HttpAdBeaconSink`] sends the impression beacon.
//!
//! This uses the **blocking** reqwest client, so callers must run these off any
//! async runtime thread (src-tauri wraps them in `spawn_blocking`).
//!
//! Domain rules (slot, lengths, https URLs) stay in `AdCreative::from_draft`;
//! this adapter only reads the wire shape, resolves the image and converts dates.
use motogp_application::{
    AdBeaconError, AdBeaconSink, AdPayload, AdSource, AdSourceError, Clock, RawAd,
};
use motogp_domain::{AdCreativeDraft, AdLocaleText};
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::time::Duration;

/// Raw URL of the ad feed on the default branch of the project repository.
pub const DEFAULT_ADS_URL: &str =
    "https://raw.githubusercontent.com/IcaroSilvaFK/motogp-launcher/main/catalog/ads.json";

/// Base that relative image paths of the feed are resolved against.
pub const DEFAULT_RAW_BASE_URL: &str =
    "https://raw.githubusercontent.com/IcaroSilvaFK/motogp-launcher/main/";

const MS_PER_DAY: u64 = 86_400_000;

/// Settings of [`HttpAdSource`] and [`HttpAdBeaconSink`]. The URLs are
/// configurable so tests can point at a local `http://127.0.0.1:<port>` server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpAdConfig {
    pub url: String,
    /// Relative image paths of the feed are appended to this base.
    pub raw_base_url: String,
    pub user_agent: String,
    pub connect_timeout: Duration,
    pub timeout: Duration,
    /// Largest accepted feed body, in bytes.
    pub max_bytes: u64,
    /// Largest accepted image body, in bytes.
    pub max_image_bytes: u64,
}

impl HttpAdConfig {
    /// Production defaults: the GitHub raw URLs, a descriptive User-Agent,
    /// 10 s to connect, 20 s in total, a 256 KiB feed cap and 200 KiB images.
    pub fn github(app_version: &str) -> Self {
        Self {
            url: DEFAULT_ADS_URL.to_string(),
            raw_base_url: DEFAULT_RAW_BASE_URL.to_string(),
            user_agent: format!(
                "motogp-launcher/{app_version} (+https://github.com/IcaroSilvaFK/motogp-launcher)"
            ),
            connect_timeout: Duration::from_secs(10),
            timeout: Duration::from_secs(20),
            max_bytes: 256 * 1024,
            max_image_bytes: 200 * 1024,
        }
    }
}

fn build_client(config: &HttpAdConfig) -> Result<Client, String> {
    // reqwest is built with `rustls-no-provider`: pick ring, as the updater does.
    // An error only means a provider is already installed, which is fine.
    let _ = rustls::crypto::ring::default_provider().install_default();
    Client::builder()
        .user_agent(config.user_agent.clone())
        .connect_timeout(config.connect_timeout)
        .timeout(config.timeout)
        .build()
        .map_err(|e| e.to_string())
}

/// A downloaded body and its `Content-Type`.
struct Download {
    bytes: Vec<u8>,
    content_type: Option<String>,
}

/// GET `url` and reads at most `limit` bytes.
fn download(client: &Client, url: &str, limit: u64) -> Result<Download, AdSourceError> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| AdSourceError::Network(e.to_string()))?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Err(AdSourceError::NotPublished);
    }
    if !status.is_success() {
        return Err(AdSourceError::Http {
            status: status.as_u16(),
        });
    }
    if response.content_length().is_some_and(|len| len > limit) {
        return Err(AdSourceError::TooLarge { limit });
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    // Read at most limit + 1 bytes: one extra byte proves the body is too big.
    let mut bytes = Vec::new();
    response
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| AdSourceError::Network(e.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(AdSourceError::TooLarge { limit });
    }
    Ok(Download {
        bytes,
        content_type,
    })
}

/// One creative of the feed plus the absolute URL of its image, still to download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedEntry {
    pub raw: RawAd,
    pub image_url: Option<String>,
}

/// [`AdSource`] that downloads the feed and its images over HTTP(S) with rustls.
#[derive(Debug)]
pub struct HttpAdSource<K> {
    client: Client,
    config: HttpAdConfig,
    clock: K,
}

impl<K: Clock> HttpAdSource<K> {
    /// Builds the HTTP client. Fails only if the TLS backend cannot start.
    pub fn new(config: HttpAdConfig, clock: K) -> Result<Self, AdSourceError> {
        let client = build_client(&config).map_err(AdSourceError::Network)?;
        Ok(Self {
            client,
            config,
            clock,
        })
    }

    /// Downloads `image_url` and encodes it as a data URI. Any failure fails the
    /// whole feed: a half-broken feed is a publishing mistake and the last good
    /// one is safer.
    fn image_data_uri(&self, image_url: &str) -> Result<String, AdSourceError> {
        let image = download(&self.client, image_url, self.config.max_image_bytes).map_err(
            |e| match e {
                AdSourceError::NotPublished => AdSourceError::Http { status: 404 },
                other => other,
            },
        )?;
        let subtype = image_subtype(image.content_type.as_deref()).ok_or_else(|| {
            AdSourceError::Invalid(format!(
                "image {image_url} is not a PNG, JPEG or WebP (content type {:?})",
                image.content_type
            ))
        })?;
        Ok(format!(
            "data:image/{subtype};base64,{}",
            base64_encode(&image.bytes)
        ))
    }
}

impl<K: Clock + Send + Sync> AdSource for HttpAdSource<K> {
    fn fetch(&self) -> Result<AdPayload, AdSourceError> {
        let feed = download(&self.client, &self.config.url, self.config.max_bytes)?;
        let entries = parse_ads(
            &feed.bytes,
            self.clock.now_millis(),
            &self.config.raw_base_url,
        )?;
        // The same image may serve several creatives: download it once.
        let mut images: HashMap<String, String> = HashMap::new();
        let mut creatives = Vec::with_capacity(entries.len());
        for FeedEntry { raw, image_url } in entries {
            let raw = match (raw, image_url) {
                (RawAd::Draft(mut draft), Some(url)) => {
                    if !images.contains_key(&url) {
                        images.insert(url.clone(), self.image_data_uri(&url)?);
                    }
                    draft.image_data_uri = images.get(&url).cloned();
                    RawAd::Draft(draft)
                }
                (raw, _) => raw,
            };
            creatives.push(raw);
        }
        Ok(AdPayload { creatives })
    }
}

/// `image/png`, `image/jpeg` or `image/webp` (parameters ignored) to the data
/// URI subtype. SVG and anything else is refused.
fn image_subtype(content_type: Option<&str>) -> Option<&'static str> {
    let essence = content_type?.split(';').next()?.trim().to_ascii_lowercase();
    match essence.as_str() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpeg"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

/// Standard base64 with padding.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = chunk.iter().enumerate().fold(0u32, |acc, (i, byte)| {
            acc | (u32::from(*byte) << (16 - 8 * i))
        });
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((group >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Wire shape of one creative. Unknown fields are ignored here: strictness
/// lives in the JSON schema and in the strict feed-file test.
#[derive(Debug, Deserialize)]
struct CreativeWire {
    id: String,
    slot: String,
    title: String,
    body: String,
    image: Option<String>,
    click_url: String,
    sponsor_label: String,
    impression_url: Option<String>,
    starts_at: Option<String>,
    expires_at: Option<String>,
    #[serde(default)]
    locales: BTreeMap<String, LocaleWire>,
}

/// Localized text of one creative; unknown fields are ignored like in [`CreativeWire`].
#[derive(Debug, Deserialize)]
struct LocaleWire {
    title: Option<String>,
    body: Option<String>,
    sponsor_label: Option<String>,
}

/// Parses the feed document. The top level must be an object with `version` 1
/// and a `creatives` array; each creative is parsed on its own, so one bad
/// creative becomes [`RawAd::Unparseable`] instead of failing the whole feed.
/// A creative whose `starts_at` is later than today (UTC) is skipped.
pub fn parse_ads(
    bytes: &[u8],
    now_ms: u64,
    raw_base_url: &str,
) -> Result<Vec<FeedEntry>, AdSourceError> {
    let invalid = |detail: &str| AdSourceError::Invalid(detail.to_string());
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| AdSourceError::Invalid(e.to_string()))?;
    let object = root
        .as_object()
        .ok_or_else(|| invalid("top level is not an object"))?;
    if object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(invalid("version is missing or not 1"));
    }
    let creatives = object
        .get("creatives")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("creatives is missing or not an array"))?;
    Ok(creatives
        .iter()
        .filter_map(|value| {
            match serde_json::from_value::<CreativeWire>(value.clone())
                .map_err(|e| e.to_string())
                .and_then(|wire| convert(wire, now_ms, raw_base_url))
            {
                Ok(Some(entry)) => Some(entry),
                Ok(None) => None,
                Err(reason) => Some(FeedEntry {
                    raw: RawAd::Unparseable { reason },
                    image_url: None,
                }),
            }
        })
        .collect())
}

/// `Ok(None)` when the creative starts in the future.
fn convert(
    wire: CreativeWire,
    now_ms: u64,
    raw_base_url: &str,
) -> Result<Option<FeedEntry>, String> {
    if let Some(starts_at) = &wire.starts_at {
        if day_start_ms(starts_at)? > now_ms {
            return Ok(None);
        }
    }
    let expires_at_ms = wire
        .expires_at
        .as_deref()
        .map(|date| day_start_ms(date).map(|start| start + MS_PER_DAY - 1))
        .transpose()?;
    let image_url = wire
        .image
        .as_deref()
        .map(|image| resolve_image(image, raw_base_url))
        .transpose()?;
    Ok(Some(FeedEntry {
        raw: RawAd::Draft(AdCreativeDraft {
            id: wire.id,
            slot: wire.slot,
            title: wire.title,
            body: wire.body,
            image_data_uri: None,
            click_url: wire.click_url,
            sponsor_label: wire.sponsor_label,
            impression_url: wire.impression_url,
            expires_at_ms,
            locales: wire
                .locales
                .into_iter()
                .map(|(locale, text)| {
                    (
                        locale,
                        AdLocaleText {
                            title: text.title,
                            body: text.body,
                            sponsor_label: text.sponsor_label,
                        },
                    )
                })
                .collect(),
        }),
        image_url,
    }))
}

/// An `https://` URL is used as is; a plain relative path is appended to
/// `raw_base_url`. Anything else (http, `data:`, absolute paths, `..`) is refused.
fn resolve_image(image: &str, raw_base_url: &str) -> Result<String, String> {
    if image.starts_with("https://") {
        return Ok(image.to_string());
    }
    let escapes = image.is_empty()
        || image.contains(':')
        || image.contains('\\')
        || image.starts_with('/')
        || image.split('/').any(|part| part == "..");
    if escapes {
        return Err(format!(
            "image {image:?} is not a relative path or https URL"
        ));
    }
    Ok(format!("{}/{image}", raw_base_url.trim_end_matches('/')))
}

/// Unix milliseconds of 00:00:00 UTC of a `YYYY-MM-DD` date.
fn day_start_ms(date: &str) -> Result<u64, String> {
    let bad = || format!("{date:?} is not a real YYYY-MM-DD date");
    let mut parts = date.split('-');
    let (Some(y), Some(m), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad());
    };
    let digits =
        |text: &str, len: usize| text.len() == len && text.bytes().all(|b| b.is_ascii_digit());
    if !(digits(y, 4) && digits(m, 2) && digits(d, 2)) {
        return Err(bad());
    }
    let (year, month, day) = (
        y.parse::<i64>().map_err(|_| bad())?,
        m.parse::<i64>().map_err(|_| bad())?,
        d.parse::<i64>().map_err(|_| bad())?,
    );
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return Err(bad());
    }
    u64::try_from(days_from_civil(year, month, day))
        .map(|days| days * MS_PER_DAY)
        .map_err(|_| bad())
}

fn days_in_month(year: i64, month: i64) -> i64 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// [`AdBeaconSink`]: one HTTPS GET to the sponsor's impression URL.
#[derive(Debug)]
pub struct HttpAdBeaconSink {
    client: Client,
    require_https: bool,
}

impl HttpAdBeaconSink {
    /// Uses the User-Agent and timeouts of `config`.
    pub fn new(config: &HttpAdConfig) -> Result<Self, AdBeaconError> {
        Ok(Self {
            client: build_client(config).map_err(AdBeaconError)?,
            require_https: true,
        })
    }
}

impl AdBeaconSink for HttpAdBeaconSink {
    /// 2xx is success. The body is never read. A non-https URL is refused
    /// without any request.
    fn send_impression(&self, impression_url: &str) -> Result<(), AdBeaconError> {
        if self.require_https && !impression_url.starts_with("https://") {
            return Err(AdBeaconError(format!(
                "refusing a non-https impression URL: {impression_url}"
            )));
        }
        let response = self
            .client
            .get(impression_url)
            .send()
            .map_err(|e| AdBeaconError(e.to_string()))?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(AdBeaconError(format!("unexpected HTTP status {status}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    /// One canned response.
    struct Canned {
        head: String,
        body: Vec<u8>,
    }

    /// Minimal one-shot-per-connection server; returns the base URL (with a
    /// trailing slash) and the raw request heads seen, in order.
    fn serve(responses: Vec<Canned>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for canned in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                log.lock().unwrap().push(read_head(&mut stream));
                let _ = stream.write_all(canned.head.as_bytes());
                let _ = stream.write_all(&canned.body);
                let _ = stream.flush();
            }
        });
        (base, seen)
    }

    fn read_head(stream: &mut TcpStream) -> String {
        let mut data = Vec::new();
        let mut byte = [0u8; 1];
        while !data.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).unwrap_or(0) == 0 {
                break;
            }
            data.push(byte[0]);
        }
        String::from_utf8_lossy(&data).to_string()
    }

    fn ok(body: &[u8], content_type: &str) -> Canned {
        Canned {
            head: format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            ),
            body: body.to_vec(),
        }
    }

    fn json(body: &str) -> Canned {
        ok(body.as_bytes(), "application/json")
    }

    fn png(bytes: &[u8]) -> Canned {
        ok(bytes, "image/png")
    }

    fn status(code: &str) -> Canned {
        Canned {
            head: format!("HTTP/1.1 {code}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
            body: Vec::new(),
        }
    }

    #[derive(Clone, Copy)]
    struct FixedClock(u64);

    impl Clock for FixedClock {
        fn now_millis(&self) -> u64 {
            self.0
        }
    }

    /// 2026-06-15T12:00:00Z.
    const NOW: u64 = 1_781_524_800_000;

    fn config(base: &str) -> HttpAdConfig {
        HttpAdConfig {
            url: format!("{base}ads.json"),
            raw_base_url: base.to_string(),
            user_agent: "test-agent/1".to_string(),
            connect_timeout: Duration::from_secs(2),
            timeout: Duration::from_secs(5),
            max_bytes: 4096,
            max_image_bytes: 64,
        }
    }

    fn source(config: HttpAdConfig) -> HttpAdSource<FixedClock> {
        HttpAdSource::new(config, FixedClock(NOW)).unwrap()
    }

    fn creative_json(id: &str, extra: &str) -> String {
        format!(
            r#"{{"id":"{id}","slot":"about","title":"T","body":"B","image":"ads/{id}.png","click_url":"https://example.com/c","sponsor_label":"S"{extra}}}"#
        )
    }

    fn feed(creatives: &[String]) -> String {
        format!(r#"{{"version":1,"creatives":[{}]}}"#, creatives.join(","))
    }

    fn drafts(payload: AdPayload) -> Vec<AdCreativeDraft> {
        payload
            .creatives
            .into_iter()
            .map(|raw| match raw {
                RawAd::Draft(draft) => draft,
                other => panic!("expected a draft, got {other:?}"),
            })
            .collect()
    }

    // ---------- configuration ----------

    #[test]
    fn github_config_has_the_documented_defaults() {
        let config = HttpAdConfig::github("1.2.3");
        assert_eq!(config.url, DEFAULT_ADS_URL);
        assert_eq!(config.raw_base_url, DEFAULT_RAW_BASE_URL);
        assert_eq!(
            config.user_agent,
            "motogp-launcher/1.2.3 (+https://github.com/IcaroSilvaFK/motogp-launcher)"
        );
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.timeout, Duration::from_secs(20));
        assert_eq!(config.max_bytes, 256 * 1024);
        assert_eq!(config.max_image_bytes, 200 * 1024);
    }

    // ---------- fetching ----------

    #[test]
    fn fetches_the_feed_and_inlines_the_image_as_a_data_uri() {
        let body = feed(&[creative_json("one", "")]);
        let (base, seen) = serve(vec![json(&body), png(b"foobar")]);
        let payload = source(config(&base)).fetch().unwrap();
        let drafts = drafts(payload);
        assert_eq!(drafts.len(), 1);
        assert_eq!(
            drafts[0].image_data_uri.as_deref(),
            Some("data:image/png;base64,Zm9vYmFy")
        );
        let requests = seen.lock().unwrap();
        assert!(requests[0].starts_with("GET /ads.json "));
        assert!(requests[0]
            .to_lowercase()
            .contains("user-agent: test-agent/1"));
        assert!(requests[1].starts_with("GET /ads/one.png "));
    }

    #[test]
    fn a_shared_image_is_downloaded_once() {
        let shared =
            |id: &str| creative_json(id, "").replace(&format!("ads/{id}.png"), "ads/s.png");
        let body = feed(&[shared("one"), shared("two")]);
        let (base, seen) = serve(vec![json(&body), png(b"abc")]);
        let drafts = drafts(source(config(&base)).fetch().unwrap());
        assert_eq!(drafts[0].image_data_uri, drafts[1].image_data_uri);
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn unparseable_and_future_creatives_download_no_image() {
        let future = creative_json("later", r#","starts_at":"2026-06-16""#);
        let body = feed(&[future, r#""oops""#.to_string()]);
        let (base, seen) = serve(vec![json(&body)]);
        let payload = source(config(&base)).fetch().unwrap();
        assert_eq!(payload.creatives.len(), 1);
        assert!(matches!(payload.creatives[0], RawAd::Unparseable { .. }));
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_creative_keeps_its_localized_text() {
        let extra = r#","locales":{"pt-BR":{"title":"Titulo","sponsor_label":"Patrocinio"}}"#;
        let body = feed(&[creative_json("one", extra)]);
        let (base, _) = serve(vec![json(&body), png(b"foobar")]);
        let drafts = drafts(source(config(&base)).fetch().unwrap());
        let pt = &drafts[0].locales["pt-BR"];
        assert_eq!(pt.title.as_deref(), Some("Titulo"));
        assert_eq!(pt.body, None);
        assert_eq!(pt.sponsor_label.as_deref(), Some("Patrocinio"));
    }

    #[test]
    fn a_creative_without_an_image_stays_a_draft_without_one() {
        let body = feed(&[creative_json("one", "").replace(r#""image":"ads/one.png","#, "")]);
        let (base, _) = serve(vec![json(&body)]);
        let drafts = drafts(source(config(&base)).fetch().unwrap());
        assert_eq!(drafts[0].image_data_uri, None);
    }

    #[test]
    fn a_404_feed_is_not_published() {
        let (base, _) = serve(vec![status("404 Not Found")]);
        assert_eq!(
            source(config(&base)).fetch(),
            Err(AdSourceError::NotPublished)
        );
    }

    #[test]
    fn other_statuses_map_to_http_error() {
        let (base, _) = serve(vec![status("500 Internal Server Error")]);
        assert_eq!(
            source(config(&base)).fetch(),
            Err(AdSourceError::Http { status: 500 })
        );
    }

    #[test]
    fn connection_refused_is_a_network_error() {
        // Bind then drop to get a port nobody listens on.
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let result = source(config(&format!("http://127.0.0.1:{port}/"))).fetch();
        assert!(matches!(result, Err(AdSourceError::Network(_))));
    }

    #[test]
    fn a_feed_over_the_cap_is_too_large() {
        let body = feed(&[creative_json("one", "")]);
        let (base, _) = serve(vec![json(&body)]);
        let mut small = config(&base);
        small.max_bytes = 50;
        assert_eq!(
            source(small).fetch(),
            Err(AdSourceError::TooLarge { limit: 50 })
        );
    }

    #[test]
    fn a_streamed_feed_over_the_cap_is_too_large() {
        // No Content-Length: the body ends when the server closes the connection.
        let canned = Canned {
            head: "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_string(),
            body: feed(&[creative_json("one", "")]).into_bytes(),
        };
        let (base, _) = serve(vec![canned]);
        let mut small = config(&base);
        small.max_bytes = 50;
        assert_eq!(
            source(small).fetch(),
            Err(AdSourceError::TooLarge { limit: 50 })
        );
    }

    #[test]
    fn invalid_json_is_invalid() {
        let (base, _) = serve(vec![json("not json")]);
        assert!(matches!(
            source(config(&base)).fetch(),
            Err(AdSourceError::Invalid(_))
        ));
    }

    #[test]
    fn an_image_over_the_cap_fails_the_whole_fetch() {
        let body = feed(&[creative_json("one", "")]);
        let (base, _) = serve(vec![json(&body), png(&[7u8; 65])]);
        assert_eq!(
            source(config(&base)).fetch(),
            Err(AdSourceError::TooLarge { limit: 64 })
        );
    }

    #[test]
    fn a_streamed_image_over_the_cap_fails_the_whole_fetch() {
        let body = feed(&[creative_json("one", "")]);
        let image = Canned {
            head: "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nConnection: close\r\n\r\n"
                .to_string(),
            body: vec![7u8; 65],
        };
        let (base, _) = serve(vec![json(&body), image]);
        assert_eq!(
            source(config(&base)).fetch(),
            Err(AdSourceError::TooLarge { limit: 64 })
        );
    }

    #[test]
    fn a_non_raster_image_fails_the_whole_fetch() {
        for content_type in ["image/svg+xml", "text/plain", "image/gif"] {
            let body = feed(&[creative_json("one", "")]);
            let (base, _) = serve(vec![json(&body), ok(b"<svg/>", content_type)]);
            assert!(
                matches!(
                    source(config(&base)).fetch(),
                    Err(AdSourceError::Invalid(_))
                ),
                "{content_type}"
            );
        }
    }

    #[test]
    fn an_image_without_a_content_type_fails_the_whole_fetch() {
        let body = feed(&[creative_json("one", "")]);
        let bare = Canned {
            head: "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n".to_string(),
            body: vec![1],
        };
        let (base, _) = serve(vec![json(&body), bare]);
        assert!(matches!(
            source(config(&base)).fetch(),
            Err(AdSourceError::Invalid(_))
        ));
    }

    #[test]
    fn image_download_errors_are_mapped() {
        let body = feed(&[creative_json("one", "")]);
        let (base, _) = serve(vec![json(&body), status("404 Not Found")]);
        assert_eq!(
            source(config(&base)).fetch(),
            Err(AdSourceError::Http { status: 404 })
        );
        let (base, _) = serve(vec![json(&body), status("503 Service Unavailable")]);
        assert_eq!(
            source(config(&base)).fetch(),
            Err(AdSourceError::Http { status: 503 })
        );
    }

    #[test]
    fn jpeg_and_webp_are_accepted_with_content_type_parameters() {
        for (content_type, prefix) in [
            ("image/jpeg", "data:image/jpeg;base64,"),
            ("IMAGE/WebP; charset=binary", "data:image/webp;base64,"),
        ] {
            let body = feed(&[creative_json("one", "")]);
            let (base, _) = serve(vec![json(&body), ok(b"x", content_type)]);
            let drafts = drafts(source(config(&base)).fetch().unwrap());
            assert_eq!(
                drafts[0].image_data_uri.as_deref(),
                Some(format!("{prefix}eA==").as_str())
            );
        }
    }

    // ---------- parse_ads ----------

    const BASE: &str = "https://raw.example.com/repo/main/";

    fn parse(body: &str) -> Vec<FeedEntry> {
        parse_ads(body.as_bytes(), NOW, BASE).unwrap()
    }

    fn only_draft(body: &str) -> (AdCreativeDraft, Option<String>) {
        let mut entries = parse(body);
        assert_eq!(entries.len(), 1, "{entries:?}");
        let FeedEntry { raw, image_url } = entries.remove(0);
        let RawAd::Draft(draft) = raw else {
            panic!("draft expected");
        };
        (draft, image_url)
    }

    #[test]
    fn parse_rejects_wrong_top_level_shapes() {
        for body in [
            "[]",
            r#"{"creatives":[]}"#,
            r#"{"version":2,"creatives":[]}"#,
            r#"{"version":"1","creatives":[]}"#,
            r#"{"version":1}"#,
            r#"{"version":1,"creatives":{}}"#,
        ] {
            assert!(
                matches!(
                    parse_ads(body.as_bytes(), NOW, BASE),
                    Err(AdSourceError::Invalid(_))
                ),
                "{body}"
            );
        }
    }

    #[test]
    fn parse_reads_snake_case_fields_and_optional_ones() {
        let extra = r#","impression_url":"https://example.com/p","starts_at":"2026-06-15","expires_at":"2026-12-31""#;
        let (draft, image_url) = only_draft(&feed(&[creative_json("one", extra)]));
        assert_eq!(draft.id, "one");
        assert_eq!(draft.slot, "about");
        assert_eq!(draft.title, "T");
        assert_eq!(draft.body, "B");
        assert_eq!(draft.click_url, "https://example.com/c");
        assert_eq!(draft.sponsor_label, "S");
        assert_eq!(
            draft.impression_url.as_deref(),
            Some("https://example.com/p")
        );
        assert_eq!(draft.expires_at_ms, Some(1_798_761_599_999));
        assert_eq!(draft.image_data_uri, None);
        assert_eq!(
            image_url.as_deref(),
            Some("https://raw.example.com/repo/main/ads/one.png")
        );
        let (plain, _) = only_draft(&feed(&[creative_json("two", "")]));
        assert_eq!(plain.impression_url, None);
        assert_eq!(plain.expires_at_ms, None);
    }

    #[test]
    fn parse_keeps_malformed_creatives_as_unparseable() {
        let wrong_type = creative_json("a", "").replace(r#""title":"T""#, r#""title":5"#);
        let missing = creative_json("b", "").replace(r#""click_url":"https://example.com/c","#, "");
        let body = feed(&[
            creative_json("ok", ""),
            wrong_type,
            missing,
            r#""oops""#.to_string(),
        ]);
        let entries = parse(&body);
        assert_eq!(entries.len(), 4);
        assert!(matches!(entries[0].raw, RawAd::Draft(_)));
        for entry in &entries[1..] {
            assert!(matches!(entry.raw, RawAd::Unparseable { .. }), "{entry:?}");
            assert_eq!(entry.image_url, None);
        }
    }

    #[test]
    fn domain_rules_are_not_duplicated_in_the_adapter() {
        // Unknown slot and http click URL are the domain's business.
        let odd = creative_json("odd", "")
            .replace(r#""slot":"about""#, r#""slot":"nowhere""#)
            .replace("https://example.com/c", "http://example.com/c");
        let (draft, _) = only_draft(&feed(&[odd]));
        assert_eq!(draft.slot, "nowhere");
        assert_eq!(draft.click_url, "http://example.com/c");
    }

    #[test]
    fn a_future_start_is_skipped_and_today_or_past_is_kept() {
        let ids = |starts: &str| -> usize {
            parse(&feed(&[creative_json(
                "one",
                &format!(r#","starts_at":"{starts}""#),
            )]))
            .len()
        };
        assert_eq!(ids("2026-06-16"), 0);
        assert_eq!(ids("2026-06-15"), 1);
        assert_eq!(ids("2025-01-01"), 1);
    }

    #[test]
    fn expiry_is_the_end_of_that_day_in_utc() {
        let expiry = |date: &str| {
            only_draft(&feed(&[creative_json(
                "one",
                &format!(r#","expires_at":"{date}""#),
            )]))
            .0
            .expires_at_ms
        };
        assert_eq!(expiry("1970-01-01"), Some(86_399_999));
        assert_eq!(expiry("2026-12-31"), Some(1_798_761_599_999));
        // A leap day is one day before March 1st.
        let leap = expiry("2028-02-29").unwrap();
        assert_eq!(expiry("2028-03-01"), Some(leap + 86_400_000));
        assert!(expiry("2000-02-29").is_some());
    }

    #[test]
    fn unreal_or_malformed_dates_are_unparseable() {
        for date in [
            "2026-02-30",
            "2100-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "2026-1-1",
            "26-01-01",
            "2026/01/01",
            "2026-01-01-01",
            "20x6-01-01",
            "",
            "0000-01-01",
        ] {
            for field in ["expires_at", "starts_at"] {
                let entries = parse(&feed(&[creative_json(
                    "one",
                    &format!(r#","{field}":"{date}""#),
                )]));
                assert!(
                    matches!(entries[0].raw, RawAd::Unparseable { .. }),
                    "{field} {date:?}"
                );
            }
        }
    }

    #[test]
    fn images_resolve_against_the_base_or_stay_https() {
        let image = |value: &str| {
            let escaped = value.replace('\\', "\\\\");
            let entries = parse(&feed(&[
                creative_json("one", "").replace("ads/one.png", &escaped)
            ]));
            match &entries[0].raw {
                RawAd::Draft(_) => Ok(entries[0].image_url.clone().unwrap()),
                RawAd::Unparseable { .. } => Err(()),
            }
        };
        assert_eq!(
            image("catalog/ads/x.png"),
            Ok("https://raw.example.com/repo/main/catalog/ads/x.png".to_string())
        );
        assert_eq!(
            image("https://cdn.example.com/x.png"),
            Ok("https://cdn.example.com/x.png".to_string())
        );
        for refused in [
            "http://cdn.example.com/x.png",
            "data:image/png;base64,AAAA",
            "../secret.png",
            "a/../../b.png",
            "/etc/passwd",
            "..\\x.png",
            "file:///c:/x.png",
            "",
        ] {
            assert_eq!(image(refused), Err(()), "{refused:?}");
        }
    }

    #[test]
    fn a_base_without_a_trailing_slash_still_resolves() {
        assert_eq!(
            resolve_image("a.png", "https://h.example/base"),
            Ok("https://h.example/base/a.png".to_string())
        );
    }

    // ---------- helpers ----------

    #[test]
    fn base64_matches_the_rfc_4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(input.as_bytes()), expected, "{input:?}");
        }
        assert_eq!(base64_encode(&[0xFF, 0xFE, 0xFD]), "//79");
    }

    #[test]
    fn only_png_jpeg_and_webp_are_images() {
        assert_eq!(image_subtype(Some("image/png")), Some("png"));
        assert_eq!(image_subtype(Some(" image/jpeg ;q=1")), Some("jpeg"));
        assert_eq!(image_subtype(Some("image/svg+xml")), None);
        assert_eq!(image_subtype(None), None);
    }

    // ---------- beacon ----------

    fn beacon(require_https: bool) -> HttpAdBeaconSink {
        let mut sink = HttpAdBeaconSink::new(&config("http://unused/")).unwrap();
        sink.require_https = require_https;
        sink
    }

    #[test]
    fn a_2xx_beacon_is_ok_and_sends_the_user_agent() {
        let (base, seen) = serve(vec![status("204 No Content")]);
        assert_eq!(beacon(false).send_impression(&format!("{base}p")), Ok(()));
        let request = seen.lock().unwrap()[0].to_lowercase();
        assert!(request.starts_with("get /p "));
        assert!(request.contains("user-agent: test-agent/1"));
    }

    #[test]
    fn a_non_2xx_beacon_is_an_error() {
        let (base, _) = serve(vec![status("500 Internal Server Error")]);
        let error = beacon(false)
            .send_impression(&format!("{base}p"))
            .unwrap_err();
        assert!(error.0.contains("500"));
    }

    #[test]
    fn a_refused_beacon_connection_is_an_error() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert!(beacon(false)
            .send_impression(&format!("http://127.0.0.1:{port}/p"))
            .is_err());
    }

    #[test]
    fn a_non_https_beacon_is_refused_without_a_request() {
        let (base, seen) = serve(vec![status("204 No Content")]);
        let error = beacon(true)
            .send_impression(&format!("{base}p"))
            .unwrap_err();
        assert!(error.0.contains("non-https"));
        assert!(seen.lock().unwrap().is_empty());
    }
}
