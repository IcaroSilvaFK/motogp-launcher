#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploy_preserves_p_suffix() {
        let name = deployed_name(0, "a1b2c3d4e5f6", "Ducati_P.pak");
        assert_eq!(name, "000_a1b2c3d4e5f6_Ducati_P.pak");
        assert!(name.ends_with("_Ducati_P.pak"));
    }

    #[test]
    fn index_is_zero_padded_to_three_digits_and_grows_beyond() {
        assert_eq!(deployed_name(7, "a", "x.pak"), "007_a_x.pak");
        assert_eq!(deployed_name(1234, "a", "x.pak"), "1234_a_x.pak");
    }

    #[test]
    fn safe_names_are_simple_relative_file_names() {
        for name in ["A.pak", "000_a_B_P.pak", "with space.pak", "a.b.c"] {
            assert!(is_safe_file_name(name), "{name:?} should be safe");
        }
    }

    #[test]
    fn unsafe_names_are_rejected() {
        for name in [
            "",
            ".",
            "..",
            "../a.pak",
            "..\\a.pak",
            "a/b.pak",
            "a\\b.pak",
            "/abs.pak",
            "C:\\abs.pak",
            "C:abs.pak",
            "a\0b",
            "trailing.",
            "trailing ",
        ] {
            assert!(!is_safe_file_name(name), "{name:?} should be unsafe");
        }
    }

    #[test]
    fn default_manifest_is_empty() {
        let deployed = Deployed::default();
        assert!(deployed.is_empty());
        assert_eq!(deployed.tracked().count(), 0);
    }

    #[test]
    fn tracked_lists_deployed_and_pending_files() {
        let deployed = Deployed {
            mods_dir: "mods".into(),
            files: vec!["a".into()],
            pending: vec!["b".into()],
        };
        assert!(!deployed.is_empty());
        assert_eq!(deployed.tracked().collect::<Vec<_>>(), vec!["a", "b"]);
    }
}
