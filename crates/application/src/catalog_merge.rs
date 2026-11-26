//! The catalog the launcher shows is the GitHub one (primary) plus the one the
//! community server hosts (issue #130). [`MergedCatalogSource`] fetches both
//! and presents them as one [`CatalogSource`], so [`crate::CatalogService`] and
//! its cache stay unchanged.
//!
//! Rules:
//! - an entry of the community wins over the GitHub entry with the same id (the
//!   GitHub one is dropped silently, it is not a duplicate warning);
//! - any community failure (network, invalid, newer schema, rate limit) means
//!   "GitHub only";
//! - a GitHub failure is returned as it is, so the service falls back to its
//!   cache and backoff;
//! - the two ETags travel packed into one string. The last raw document of each
//!   source stays in memory, so a part that answers "not modified" is reused.
use crate::{CatalogSource, CatalogSourceError, RawCatalog, RawEntry, SourceFetch};
use motogp_domain::SUPPORTED_CATALOG_SCHEMA;
use std::collections::HashSet;
use std::sync::Mutex;

/// What was last received from one source.
#[derive(Debug, Clone)]
struct Part {
    catalog: RawCatalog,
    etag: Option<String>,
}

#[derive(Debug, Default)]
struct Memory {
    github: Option<Part>,
    community: Option<Part>,
}

/// GitHub catalog (primary) merged with the community server catalog.
pub struct MergedCatalogSource<G, C> {
    github: G,
    community: C,
    memory: Mutex<Memory>,
}

impl<G: CatalogSource, C: CatalogSource> MergedCatalogSource<G, C> {
    pub fn new(github: G, community: C) -> Self {
        Self {
            github,
            community,
            memory: Mutex::default(),
        }
    }
}

/// Packs the two part ETags into one string: `<len of github etag>:<github><community>`.
/// `None` when neither part has an ETag (nothing to revalidate with).
fn pack(github: Option<&str>, community: Option<&str>) -> Option<String> {
    if github.is_none() && community.is_none() {
        return None;
    }
    let github = github.unwrap_or("");
    Some(format!(
        "{}:{github}{}",
        github.len(),
        community.unwrap_or("")
    ))
}

/// Inverse of [`pack`], used to compare a combined ETag with a fresh one.
/// Kept for the unit tests: production code only compares packed strings.
#[cfg(test)]
fn unpack(packed: &str) -> Option<(Option<String>, Option<String>)> {
    let (len, rest) = packed.split_once(':')?;
    let len: usize = len.parse().ok()?;
    if !rest.is_char_boundary(len) {
        return None;
    }
    let (github, community) = rest.split_at(len);
    let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
    Some((some(github), some(community)))
}

/// GitHub entries minus the ids the community also has, then the community's.
fn merge(github: &RawCatalog, community: Option<&RawCatalog>) -> RawCatalog {
    let Some(community) = community else {
        return github.clone();
    };
    let ids: HashSet<&str> = community
        .entries
        .iter()
        .filter_map(|entry| match entry {
            RawEntry::Draft(draft) => Some(draft.id.as_str()),
            RawEntry::Unparseable { .. } => None,
        })
        .collect();
    let mut entries: Vec<RawEntry> = github
        .entries
        .iter()
        .filter(|entry| match entry {
            RawEntry::Draft(draft) => !ids.contains(draft.id.as_str()),
            RawEntry::Unparseable { .. } => true,
        })
        .cloned()
        .collect();
    entries.extend(community.entries.iter().cloned());
    RawCatalog {
        schema_version: github.schema_version,
        entries,
    }
}

/// A fetch of one source, reduced to what the merge needs.
enum PartFetch {
    Unchanged,
    Fresh(Part),
}

fn fetch_part(
    source: &dyn CatalogSource,
    held: Option<&Part>,
) -> Result<PartFetch, CatalogSourceError> {
    // Only revalidate a copy we can actually reuse.
    let etag = held.and_then(|part| part.etag.as_deref());
    match source.fetch(etag)? {
        SourceFetch::NotModified if held.is_some() => Ok(PartFetch::Unchanged),
        SourceFetch::NotModified => Err(CatalogSourceError::Invalid(
            "not modified, but nothing was cached".to_string(),
        )),
        SourceFetch::Fetched { catalog, etag } => Ok(PartFetch::Fresh(Part { catalog, etag })),
    }
}

impl<G: CatalogSource, C: CatalogSource> CatalogSource for MergedCatalogSource<G, C> {
    fn fetch(&self, etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
        let (held_github, held_community) = {
            let memory = self.memory.lock().unwrap_or_else(|e| e.into_inner());
            (memory.github.clone(), memory.community.clone())
        };
        // Both servers are asked at the same time: an offline community server
        // must not add its connect timeout to every refresh.
        let (github, community) = std::thread::scope(|scope| {
            let community = scope.spawn(|| fetch_part(&self.community, held_community.as_ref()));
            let github = fetch_part(&self.github, held_github.as_ref());
            let community = community
                .join()
                .unwrap_or_else(|_| Err(CatalogSourceError::Invalid("community panicked".into())));
            (github, community)
        });

        let github = match github? {
            PartFetch::Unchanged => held_github.expect("unchanged implies a held part"),
            PartFetch::Fresh(part) => part,
        };
        // A community failure, or a schema this launcher cannot read, hides the community only.
        let community = match community {
            Ok(PartFetch::Unchanged) => held_community,
            Ok(PartFetch::Fresh(part))
                if part.catalog.schema_version <= SUPPORTED_CATALOG_SCHEMA =>
            {
                Some(part)
            }
            Ok(PartFetch::Fresh(_)) | Err(_) => None,
        };

        let combined = pack(
            github.etag.as_deref(),
            community.as_ref().and_then(|part| part.etag.as_deref()),
        );
        let merged = merge(
            &github.catalog,
            community.as_ref().map(|part| &part.catalog),
        );
        {
            let mut memory = self.memory.lock().unwrap_or_else(|e| e.into_inner());
            memory.github = Some(github);
            memory.community = community;
        }
        // The cached copy is current when the combination did not change.
        if combined.is_some() && combined.as_deref() == etag {
            return Ok(SourceFetch::NotModified);
        }
        Ok(SourceFetch::Fetched {
            catalog: merged,
            etag: combined,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_domain::CatalogEntryDraft;
    use std::collections::VecDeque;
    use std::sync::Arc;

    fn draft(id: &str, title: &str) -> RawEntry {
        RawEntry::Draft(CatalogEntryDraft {
            id: id.into(),
            title: title.into(),
            author: "a".into(),
            game: "g".into(),
            category: "c".into(),
            description: "d".into(),
            version: "1".into(),
            updated_at: "2026-01-01".into(),
            page_url: "https://example.com".into(),
            thumbnail_url: None,
            screenshots: vec![],
            long_description: None,
            download: None,
        })
    }

    fn raw(schema: u32, entries: Vec<RawEntry>) -> RawCatalog {
        RawCatalog {
            schema_version: schema,
            entries,
        }
    }

    fn fetched(
        entries: Vec<RawEntry>,
        etag: Option<&str>,
    ) -> Result<SourceFetch, CatalogSourceError> {
        Ok(SourceFetch::Fetched {
            catalog: raw(1, entries),
            etag: etag.map(str::to_string),
        })
    }

    /// Scripted source: answers in order and records the ETag of each call.
    struct Script {
        answers: Mutex<VecDeque<Result<SourceFetch, CatalogSourceError>>>,
        etags: Mutex<Vec<Option<String>>>,
    }

    impl Script {
        fn new(answers: Vec<Result<SourceFetch, CatalogSourceError>>) -> Arc<Self> {
            Arc::new(Self {
                answers: Mutex::new(answers.into()),
                etags: Mutex::default(),
            })
        }
        fn etags(&self) -> Vec<Option<String>> {
            self.etags.lock().unwrap().clone()
        }
    }

    impl CatalogSource for Script {
        fn fetch(&self, etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
            self.etags.lock().unwrap().push(etag.map(str::to_string));
            self.answers.lock().unwrap().pop_front().expect("scripted")
        }
    }

    fn merged(
        github: &Arc<Script>,
        community: &Arc<Script>,
    ) -> MergedCatalogSource<Arc<Script>, Arc<Script>> {
        MergedCatalogSource::new(Arc::clone(github), Arc::clone(community))
    }

    fn ids(fetch: SourceFetch) -> Vec<(String, String)> {
        let SourceFetch::Fetched { catalog, .. } = fetch else {
            panic!("expected Fetched");
        };
        catalog
            .entries
            .into_iter()
            .map(|entry| match entry {
                RawEntry::Draft(d) => (d.id, d.title),
                RawEntry::Unparseable { reason } => ("?".into(), reason),
            })
            .collect()
    }

    fn etag_of(fetch: &SourceFetch) -> Option<String> {
        match fetch {
            SourceFetch::Fetched { etag, .. } => etag.clone(),
            SourceFetch::NotModified => None,
        }
    }

    #[test]
    fn the_community_entry_wins_over_the_github_one_with_the_same_id() {
        let github = Script::new(vec![fetched(
            vec![draft("brazil-livery", "GitHub"), draft("other", "Other")],
            None,
        )]);
        let community = Script::new(vec![fetched(
            vec![draft("brazil-livery", "Community")],
            None,
        )]);
        let result = merged(&github, &community).fetch(None).unwrap();
        assert_eq!(
            ids(result),
            vec![
                ("other".to_string(), "Other".to_string()),
                ("brazil-livery".to_string(), "Community".to_string()),
            ]
        );
    }

    #[test]
    fn unparseable_github_entries_are_kept() {
        let github = Script::new(vec![fetched(
            vec![
                RawEntry::Unparseable {
                    reason: "bad".into(),
                },
                draft("a", "A"),
            ],
            None,
        )]);
        let community = Script::new(vec![fetched(vec![draft("b", "B")], None)]);
        let result = merged(&github, &community).fetch(None).unwrap();
        assert_eq!(ids(result).len(), 3);
    }

    #[test]
    fn every_community_failure_falls_back_to_github_only() {
        let failures = [
            CatalogSourceError::Network("down".into()),
            CatalogSourceError::Invalid("junk".into()),
            CatalogSourceError::RateLimited {
                retry_after_secs: Some(5),
            },
            CatalogSourceError::NotPublished,
            CatalogSourceError::Http { status: 500 },
            CatalogSourceError::TooLarge { limit: 1 },
        ];
        for failure in failures {
            let github = Script::new(vec![fetched(vec![draft("a", "A")], None)]);
            let community = Script::new(vec![Err(failure.clone())]);
            let result = merged(&github, &community).fetch(None).unwrap();
            assert_eq!(
                ids(result),
                vec![("a".to_string(), "A".to_string())],
                "{failure}"
            );
        }
    }

    #[test]
    fn a_newer_community_schema_is_ignored() {
        let github = Script::new(vec![fetched(vec![draft("a", "A")], None)]);
        let community = Script::new(vec![Ok(SourceFetch::Fetched {
            catalog: raw(SUPPORTED_CATALOG_SCHEMA + 1, vec![draft("b", "B")]),
            etag: None,
        })]);
        let result = merged(&github, &community).fetch(None).unwrap();
        assert_eq!(ids(result), vec![("a".to_string(), "A".to_string())]);
    }

    #[test]
    fn a_github_failure_is_returned_as_it_is() {
        let github = Script::new(vec![Err(CatalogSourceError::Network("down".into()))]);
        let community = Script::new(vec![fetched(vec![draft("b", "B")], None)]);
        assert_eq!(
            merged(&github, &community).fetch(None),
            Err(CatalogSourceError::Network("down".into()))
        );
    }

    #[test]
    fn a_github_304_without_a_held_copy_is_invalid() {
        let github = Script::new(vec![Ok(SourceFetch::NotModified)]);
        let community = Script::new(vec![fetched(vec![], None)]);
        assert!(matches!(
            merged(&github, &community).fetch(None),
            Err(CatalogSourceError::Invalid(_))
        ));
    }

    #[test]
    fn the_combined_etag_packs_both_parts() {
        let github = Script::new(vec![fetched(vec![draft("a", "A")], Some("\"g1\""))]);
        let community = Script::new(vec![fetched(vec![draft("b", "B")], Some("\"c1\""))]);
        let result = merged(&github, &community).fetch(None).unwrap();
        let etag = etag_of(&result).unwrap();
        assert_eq!(
            unpack(&etag),
            Some((Some("\"g1\"".to_string()), Some("\"c1\"".to_string())))
        );
    }

    #[test]
    fn parts_are_revalidated_with_their_own_etags_and_reused_on_304() {
        let github = Script::new(vec![
            fetched(vec![draft("a", "A")], Some("\"g1\"")),
            Ok(SourceFetch::NotModified),
        ]);
        let community = Script::new(vec![
            fetched(vec![draft("b", "B")], Some("\"c1\"")),
            fetched(vec![draft("b", "B2")], Some("\"c2\"")),
        ]);
        let source = merged(&github, &community);
        let first = source.fetch(None).unwrap();
        let second = source.fetch(etag_of(&first).as_deref()).unwrap();
        assert_eq!(github.etags(), vec![None, Some("\"g1\"".to_string())]);
        assert_eq!(community.etags(), vec![None, Some("\"c1\"".to_string())]);
        assert_eq!(
            ids(second),
            vec![
                ("a".to_string(), "A".to_string()),
                ("b".to_string(), "B2".to_string())
            ]
        );
    }

    #[test]
    fn not_modified_only_when_both_parts_are_unchanged() {
        let github = Script::new(vec![
            fetched(vec![draft("a", "A")], Some("\"g1\"")),
            Ok(SourceFetch::NotModified),
        ]);
        let community = Script::new(vec![
            fetched(vec![draft("b", "B")], Some("\"c1\"")),
            Ok(SourceFetch::NotModified),
        ]);
        let source = merged(&github, &community);
        let first = source.fetch(None).unwrap();
        assert_eq!(
            source.fetch(etag_of(&first).as_deref()),
            Ok(SourceFetch::NotModified)
        );
    }

    #[test]
    fn an_unchanged_combination_is_not_modified_even_when_a_part_was_refetched() {
        let github = Script::new(vec![
            fetched(vec![draft("a", "A")], Some("\"g1\"")),
            fetched(vec![draft("a", "A")], Some("\"g1\"")),
        ]);
        let community = Script::new(vec![
            fetched(vec![], Some("\"c1\"")),
            fetched(vec![], Some("\"c1\"")),
        ]);
        let source = merged(&github, &community);
        let first = source.fetch(None).unwrap();
        assert_eq!(
            source.fetch(etag_of(&first).as_deref()),
            Ok(SourceFetch::NotModified)
        );
    }

    #[test]
    fn a_cached_copy_with_a_different_etag_is_replaced() {
        let github = Script::new(vec![
            fetched(vec![draft("a", "A")], Some("\"g1\"")),
            Ok(SourceFetch::NotModified),
        ]);
        let community = Script::new(vec![
            fetched(vec![draft("b", "B")], Some("\"c1\"")),
            Err(CatalogSourceError::Network("down".into())),
        ]);
        let source = merged(&github, &community);
        let first = source.fetch(None).unwrap();
        // The community went away: the answer is GitHub only, not "not modified".
        let second = source.fetch(etag_of(&first).as_deref()).unwrap();
        assert_eq!(ids(second), vec![("a".to_string(), "A".to_string())]);
    }

    #[test]
    fn a_cache_from_another_run_never_gets_a_blind_not_modified() {
        // Nothing is held in memory, so no part is revalidated: both are fetched fully.
        let github = Script::new(vec![fetched(vec![draft("a", "A")], Some("\"g1\""))]);
        let community = Script::new(vec![fetched(vec![], Some("\"c1\""))]);
        let source = merged(&github, &community);
        let old = pack(Some("\"g0\""), Some("\"c0\""));
        let result = source.fetch(old.as_deref()).unwrap();
        assert!(matches!(result, SourceFetch::Fetched { .. }));
        assert_eq!(github.etags(), vec![None]);
        assert_eq!(community.etags(), vec![None]);
    }

    #[test]
    fn no_etag_at_all_means_always_fetched() {
        let github = Script::new(vec![
            fetched(vec![draft("a", "A")], None),
            fetched(vec![draft("a", "A")], None),
        ]);
        let community = Script::new(vec![fetched(vec![], None), fetched(vec![], None)]);
        let source = merged(&github, &community);
        assert_eq!(etag_of(&source.fetch(None).unwrap()), None);
        assert!(matches!(
            source.fetch(Some("")).unwrap(),
            SourceFetch::Fetched { .. }
        ));
    }

    #[test]
    fn pack_and_unpack_round_trip_and_reject_garbage() {
        assert_eq!(pack(None, None), None);
        let packed = pack(Some("\"é\""), None).unwrap();
        assert_eq!(unpack(&packed), Some((Some("\"é\"".to_string()), None)));
        let packed = pack(None, Some("\"c\"")).unwrap();
        assert_eq!(unpack(&packed), Some((None, Some("\"c\"".to_string()))));
        assert_eq!(unpack("garbage"), None);
        assert_eq!(unpack("x:abc"), None);
        assert_eq!(unpack("1:é"), None);
    }
}
