//! Tauri-boundary mapping for profile use-case errors. Storage failures reuse
//! the issue #2 mapping; business rules get their own stable codes.
use crate::CommandError;
use motogp_application::ProfileServiceError;
use motogp_domain::ProfileError;

fn rule_code(rule: &ProfileError) -> &'static str {
    match rule {
        ProfileError::EmptyName => "empty_profile_name",
        ProfileError::NotFound => "profile_not_found",
        ProfileError::VanillaProtected => "vanilla_protected",
    }
}

impl From<&ProfileServiceError> for CommandError {
    fn from(error: &ProfileServiceError) -> Self {
        match error {
            ProfileServiceError::Storage(app) => CommandError::from(app),
            ProfileServiceError::Profile(rule) => CommandError {
                code: rule_code(rule).to_string(),
                message: rule.to_string(),
            },
        }
    }
}

impl From<ProfileServiceError> for CommandError {
    fn from(error: ProfileServiceError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::AppError;

    fn rule(error: ProfileError) -> CommandError {
        CommandError::from(ProfileServiceError::Profile(error))
    }

    #[test]
    fn maps_business_rules_to_stable_codes_and_messages() {
        let empty = rule(ProfileError::EmptyName);
        assert_eq!(empty.code, "empty_profile_name");
        assert_eq!(empty.message, "empty profile name");
        let missing = rule(ProfileError::NotFound);
        assert_eq!(missing.code, "profile_not_found");
        assert_eq!(missing.message, "profile not found");
        let vanilla = rule(ProfileError::VanillaProtected);
        assert_eq!(vanilla.code, "vanilla_protected");
        assert!(vanilla.message.contains("Vanilla"));
    }

    #[test]
    fn storage_errors_reuse_the_app_error_mapping() {
        let app = AppError::Corrupted {
            location: "profiles.json".into(),
            detail: "expected value".into(),
        };
        let mapped = CommandError::from(ProfileServiceError::Storage(app.clone()));
        assert_eq!(mapped, CommandError::from(app));
        assert_eq!(mapped.code, "data_corrupted");
    }
}
