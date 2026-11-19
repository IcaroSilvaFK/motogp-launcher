#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::InstanceLock;

    #[test]
    fn a_second_acquisition_fails_while_the_first_is_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("instance.lock");
        let first = FileInstanceLock::new(path.clone());
        let second = FileInstanceLock::new(path);

        let held = first.acquire().unwrap().expect("first instance gets it");

        assert!(second.acquire().unwrap().is_none(), "second is refused");
        drop(held);
        assert!(
            second.acquire().unwrap().is_some(),
            "free again after the first exits"
        );
    }

    #[test]
    fn an_unwritable_location_is_an_error_not_a_silent_success() {
        let dir = tempfile::tempdir().unwrap();
        let lock = FileInstanceLock::new(dir.path().join("missing").join("instance.lock"));

        assert!(lock.acquire().is_err());
    }
}
