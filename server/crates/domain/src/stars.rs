//! Stars: the identifier of a catalog mod.

use std::fmt;

/// Longest accepted mod id.
pub const MOD_ID_MAX_LEN: usize = 100;

/// Catalog slug of a mod: lowercase ASCII letters and digits, groups joined by
/// single hyphens (`^[a-z0-9]+(-[a-z0-9]+)*$`), 1 to 100 characters. The server
/// does not know the catalog, so any valid slug is accepted.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModId(String);

impl ModId {
    /// Returns `None` unless the value is a valid slug.
    pub fn parse(value: &str) -> Option<Self> {
        let valid_len = !value.is_empty() && value.len() <= MOD_ID_MAX_LEN;
        let valid = valid_len
            && value.split('-').all(|group| {
                !group.is_empty()
                    && group
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            });
        valid.then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_slugs() {
        for ok in ["a", "mod-1", "ducati-livery-2025", "0", "a-b-c", "x1-y2"] {
            assert_eq!(ModId::parse(ok).unwrap().as_str(), ok);
        }
    }

    #[test]
    fn rejects_non_slugs() {
        for bad in [
            "",
            "Not A Slug!",
            "Upper",
            "-lead",
            "trail-",
            "dou--ble",
            "under_score",
            "dot.dot",
            "slash/x",
            "acc\u{e9}nt",
            " space",
        ] {
            assert!(ModId::parse(bad).is_none(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn enforces_the_length_limit() {
        assert!(ModId::parse(&"a".repeat(100)).is_some());
        assert!(ModId::parse(&"a".repeat(101)).is_none());
    }

    #[test]
    fn displays_the_slug() {
        assert_eq!(ModId::parse("abc").unwrap().to_string(), "abc");
    }
}
