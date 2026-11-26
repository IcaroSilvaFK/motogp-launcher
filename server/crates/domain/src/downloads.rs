//! Download counts: identifiers, UTC day math and the 30-day window.

use chrono::{DateTime, Utc};

/// Seconds in a UTC day.
const SECS_PER_DAY: i64 = 86_400;
/// Number of daily buckets (today and the 29 days before).
pub const WINDOW_DAYS: i64 = 30;
const MAX_SLUG_LEN: usize = 64;
const MIN_INSTALL_ID_LEN: usize = 22;
const MAX_INSTALL_ID_LEN: usize = 128;

/// A mod id as the launcher catalog writes it: 1 to 64 characters of
/// `a-z 0-9 -`, with non-empty dash-separated parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModSlug(String);

impl ModSlug {
    pub fn parse(value: &str) -> Option<Self> {
        let valid = !value.is_empty()
            && value.len() <= MAX_SLUG_LEN
            && value.split('-').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            });
        valid.then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The random id a launcher install generates (22 to 128 characters of
/// `A-Z a-z 0-9 - _`). Its `Debug` never shows the value.
#[derive(Clone, PartialEq, Eq)]
pub struct InstallId(String);

impl InstallId {
    pub fn parse(value: &str) -> Option<Self> {
        let valid = (MIN_INSTALL_ID_LEN..=MAX_INSTALL_ID_LEN).contains(&value.len())
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        valid.then(|| Self(value.to_owned()))
    }

    /// The raw id. Only hand it to the hasher.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for InstallId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InstallId(<redacted>)")
    }
}

/// Days since 1970-01-01 UTC.
pub fn day_of(time: DateTime<Utc>) -> i64 {
    time.timestamp().div_euclid(SECS_PER_DAY)
}

/// `YYYY-MM-DD` of a day number.
pub fn date_of(day: i64) -> String {
    DateTime::from_timestamp(day.saturating_mul(SECS_PER_DAY), 0)
        .unwrap_or_default()
        .date_naive()
        .to_string()
}

/// First day of the 30-day window ending on `today`.
pub fn window_start(today: i64) -> i64 {
    today - (WINDOW_DAYS - 1)
}

/// One day of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayBucket {
    pub day: i64,
    pub count: u64,
}

/// The 30 buckets ending on `today`, oldest first, zero for days missing from
/// `counts`. Days outside the window are ignored.
pub fn fill_window(today: i64, counts: &[(i64, u64)]) -> Vec<DayBucket> {
    (window_start(today)..=today)
        .map(|day| DayBucket {
            day,
            count: counts
                .iter()
                .filter(|(d, _)| *d == day)
                .map(|(_, c)| *c)
                .sum(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_accepts_catalog_ids_and_rejects_the_rest() {
        for ok in ["a", "mod-1", "x9-y", &"a".repeat(64)] {
            assert!(ModSlug::parse(ok).is_some(), "{ok}");
        }
        for bad in [
            "",
            "-a",
            "a-",
            "a--b",
            "A",
            "a_b",
            "a b",
            "../x",
            "ü",
            &"a".repeat(65),
        ] {
            assert!(ModSlug::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn install_id_has_a_length_and_charset() {
        assert!(InstallId::parse(&"a".repeat(22)).is_some());
        assert!(InstallId::parse(&"A-_9".repeat(32)).is_some());
        assert!(InstallId::parse(&"a".repeat(21)).is_none());
        assert!(InstallId::parse(&"a".repeat(129)).is_none());
        assert!(InstallId::parse(&format!("{}!", "a".repeat(22))).is_none());
        assert!(InstallId::parse(&format!("{}=", "a".repeat(22))).is_none());
    }

    #[test]
    fn install_id_debug_is_redacted() {
        let id = InstallId::parse(&"a".repeat(22)).unwrap();
        assert!(!format!("{id:?}").contains("aaaa"));
    }

    #[test]
    fn day_math_is_utc() {
        let t = DateTime::from_timestamp(86_399, 0).unwrap();
        assert_eq!(day_of(t), 0);
        let t = DateTime::from_timestamp(86_400, 0).unwrap();
        assert_eq!(day_of(t), 1);
        assert_eq!(date_of(0), "1970-01-01");
        assert_eq!(date_of(20_000), "2024-10-04");
    }

    #[test]
    fn window_has_30_buckets_oldest_first_ending_today() {
        let today = 20_000;
        let buckets = fill_window(today, &[(today, 3), (today - 29, 1), (today - 30, 9)]);
        assert_eq!(buckets.len(), 30);
        assert_eq!(buckets[0].day, today - 29);
        assert_eq!(buckets[29].day, today);
        assert_eq!(buckets[0].count, 1);
        assert_eq!(buckets[29].count, 3);
        assert_eq!(buckets.iter().map(|b| b.count).sum::<u64>(), 4);
        assert!(buckets[1..29].iter().all(|b| b.count == 0));
    }
}
