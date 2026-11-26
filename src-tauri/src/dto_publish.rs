//! IPC DTOs of publishing mods to the community server (#130). camelCase on the
//! wire; no token, ever.
use motogp_application::{
    ModVersion, MyMod, PublishPhase, PublishProgress, PublishRequest, VersionStatus,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Argument of `community_publish_mod`. The file paths come from the native
/// file dialog.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishRequestDto {
    pub mod_id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub long_description: Option<String>,
    pub category: String,
    pub game: String,
    pub version: String,
    /// Absolute path of the `.zip`, `.7z` or `.pak` file.
    pub file: String,
    /// Absolute paths of up to 8 PNG, JPEG or WebP images.
    #[serde(default)]
    pub screenshots: Vec<String>,
    pub ownership_confirmed: bool,
}

impl From<PublishRequestDto> for PublishRequest {
    fn from(dto: PublishRequestDto) -> Self {
        PublishRequest {
            mod_id: dto.mod_id,
            title: dto.title,
            description: dto.description,
            long_description: dto.long_description,
            category: dto.category,
            game: dto.game,
            version: dto.version,
            file: PathBuf::from(dto.file),
            screenshots: dto.screenshots.into_iter().map(PathBuf::from).collect(),
            ownership_confirmed: dto.ownership_confirmed,
        }
    }
}

/// One uploaded version. `status` is `pending`, `approved` or `rejected`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModVersionDto {
    pub mod_id: String,
    pub version: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_description: Option<String>,
    pub category: String,
    pub game: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    pub status: &'static str,
    /// Why an admin rejected it. Only on rejected versions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    pub screenshot_count: u32,
    pub uploaded_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<String>,
}

impl From<ModVersion> for ModVersionDto {
    fn from(version: ModVersion) -> Self {
        Self {
            mod_id: version.mod_id,
            version: version.version,
            title: version.title,
            description: version.description,
            long_description: version.long_description,
            category: version.category,
            game: version.game,
            file_name: version.file_name,
            size: version.size,
            sha256: version.sha256,
            status: match version.status {
                VersionStatus::Pending => "pending",
                VersionStatus::Approved => "approved",
                VersionStatus::Rejected => "rejected",
            },
            rejection_reason: version.rejection_reason,
            screenshot_count: version.screenshot_count,
            uploaded_at: version.uploaded_at,
            reviewed_at: version.reviewed_at,
        }
    }
}

/// A mod of the signed-in creator with all its versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MyModDto {
    pub id: String,
    pub versions: Vec<ModVersionDto>,
}

impl From<MyMod> for MyModDto {
    fn from(mine: MyMod) -> Self {
        Self {
            id: mine.id,
            versions: mine.versions.into_iter().map(Into::into).collect(),
        }
    }
}

/// Payload of the `community-publish-progress` event. `phase` is `hashing`,
/// `uploading`, `finishing` or `done`; `sent` and `total` are bytes of the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishProgressDto {
    pub sent: u64,
    pub total: u64,
    pub phase: &'static str,
}

impl From<PublishProgress> for PublishProgressDto {
    fn from(progress: PublishProgress) -> Self {
        Self {
            sent: progress.sent,
            total: progress.total,
            phase: match progress.phase {
                PublishPhase::Hashing => "hashing",
                PublishPhase::Uploading => "uploading",
                PublishPhase::Finishing => "finishing",
                PublishPhase::Done => "done",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn version(status: VersionStatus) -> ModVersion {
        ModVersion {
            mod_id: "brazil-livery".into(),
            version: "1.0".into(),
            title: "Brazil Livery".into(),
            description: "d".into(),
            long_description: None,
            category: "liveries".into(),
            game: "MotoGP 26".into(),
            file_name: "a.zip".into(),
            size: 10,
            sha256: "h".into(),
            status,
            rejection_reason: None,
            screenshot_count: 2,
            uploaded_at: "2026-11-26T10:00:00Z".into(),
            reviewed_at: None,
        }
    }

    #[test]
    fn a_pending_version_is_camel_case_and_omits_empty_optionals() {
        assert_eq!(
            serde_json::to_value(ModVersionDto::from(version(VersionStatus::Pending))).unwrap(),
            json!({
                "modId": "brazil-livery", "version": "1.0", "title": "Brazil Livery",
                "description": "d", "category": "liveries", "game": "MotoGP 26",
                "fileName": "a.zip", "size": 10, "sha256": "h", "status": "pending",
                "screenshotCount": 2, "uploadedAt": "2026-11-26T10:00:00Z"
            })
        );
    }

    #[test]
    fn a_rejected_version_carries_the_reason() {
        let mut rejected = version(VersionStatus::Rejected);
        rejected.rejection_reason = Some("Not allowed".into());
        rejected.reviewed_at = Some("2026-11-27T10:00:00Z".into());
        rejected.long_description = Some("Long".into());
        let value = serde_json::to_value(ModVersionDto::from(rejected)).unwrap();
        assert_eq!(value["status"], "rejected");
        assert_eq!(value["rejectionReason"], "Not allowed");
        assert_eq!(value["reviewedAt"], "2026-11-27T10:00:00Z");
        assert_eq!(value["longDescription"], "Long");
        let approved = ModVersionDto::from(version(VersionStatus::Approved));
        assert_eq!(approved.status, "approved");
    }

    #[test]
    fn my_mods_nest_their_versions() {
        let dto = MyModDto::from(MyMod {
            id: "brazil-livery".into(),
            versions: vec![
                version(VersionStatus::Approved),
                version(VersionStatus::Pending),
            ],
        });
        let value = serde_json::to_value(dto).unwrap();
        assert_eq!(value["id"], "brazil-livery");
        assert_eq!(value["versions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn progress_phases_have_lowercase_wire_names() {
        for (phase, name) in [
            (PublishPhase::Hashing, "hashing"),
            (PublishPhase::Uploading, "uploading"),
            (PublishPhase::Finishing, "finishing"),
            (PublishPhase::Done, "done"),
        ] {
            let dto = PublishProgressDto::from(PublishProgress {
                phase,
                sent: 5,
                total: 10,
            });
            assert_eq!(
                serde_json::to_value(dto).unwrap(),
                json!({"sent": 5, "total": 10, "phase": name})
            );
        }
    }

    #[test]
    fn the_request_is_read_from_camel_case_with_optional_fields() {
        // The folder has accents on purpose: real users have them.
        let dto: PublishRequestDto = serde_json::from_value(json!({
            "modId": "a", "title": "t", "description": "d", "category": "c",
            "game": "g", "version": "1", "file": "C:/Usuário Padrão/a.zip",
            "ownershipConfirmed": true
        }))
        .unwrap();
        assert_eq!(dto.long_description, None);
        assert!(dto.screenshots.is_empty());
        let request = PublishRequest::from(dto);
        assert_eq!(request.mod_id, "a");
        assert_eq!(request.file, PathBuf::from("C:/Usuário Padrão/a.zip"));
        assert!(request.ownership_confirmed);
    }
}
