//! IPC DTOs of the mod stars (#119). camelCase on the wire; no token, ever.
use motogp_application::ModStats;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModStatsDto {
    pub id: String,
    pub stars: u64,
    /// Absent when the request had no valid session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starred_by_me: Option<bool>,
}

impl From<ModStats> for ModStatsDto {
    fn from(stats: ModStats) -> Self {
        Self {
            id: stats.id,
            stars: stats.stars,
            starred_by_me: stats.starred_by_me,
        }
    }
}

/// Answer of `community_mod_stats`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModStatsListDto {
    pub mods: Vec<ModStatsDto>,
}

impl From<Vec<ModStats>> for ModStatsListDto {
    fn from(mods: Vec<ModStats>) -> Self {
        Self {
            mods: mods.into_iter().map(ModStatsDto::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stats(id: &str, stars: u64, mine: Option<bool>) -> ModStats {
        ModStats {
            id: id.into(),
            stars,
            starred_by_me: mine,
        }
    }

    #[test]
    fn the_list_is_camel_case_and_omits_an_unknown_starred_flag() {
        let dto = ModStatsListDto::from(vec![stats("a", 3, Some(false)), stats("b", 1, None)]);
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            json!({"mods": [
                {"id": "a", "stars": 3, "starredByMe": false},
                {"id": "b", "stars": 1},
            ]})
        );
    }
}
