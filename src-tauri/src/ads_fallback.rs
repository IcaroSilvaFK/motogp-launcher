//! Fallback ad adapters. When the HTTP source or beacon cannot be built (see
//! `bootstrap.rs`) the app still starts: the feed is empty, so no ad is shown,
//! and impressions are dropped. Also usable as plain fakes in tests.
use motogp_application::{AdBeaconError, AdBeaconSink, AdPayload, AdSource, AdSourceError};

/// Feed that is always empty.
pub struct NullAdSource;

impl AdSource for NullAdSource {
    fn fetch(&self) -> Result<AdPayload, AdSourceError> {
        Ok(AdPayload { creatives: vec![] })
    }
}

/// Beacon that sends nothing.
pub struct NullAdBeacon;

impl AdBeaconSink for NullAdBeacon {
    fn send_impression(&self, _impression_url: &str) -> Result<(), AdBeaconError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_null_source_serves_an_empty_feed() {
        assert_eq!(NullAdSource.fetch(), Ok(AdPayload { creatives: vec![] }));
    }

    #[test]
    fn the_null_beacon_succeeds_without_sending() {
        assert_eq!(NullAdBeacon.send_impression("https://x.example/i"), Ok(()));
    }
}
