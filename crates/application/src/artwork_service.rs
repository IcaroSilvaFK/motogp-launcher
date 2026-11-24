use crate::{AppError, Artwork, GameArtworkSource};

/// Use case: the artwork of the configured game.
pub struct ArtworkService<S> {
    source: S,
}

impl<S: GameArtworkSource> ArtworkService<S> {
    pub fn new(source: S) -> Self {
        Self { source }
    }

    /// No App ID means no artwork, without touching the source.
    pub fn artwork(&self, app_id: Option<u32>) -> Result<Option<Artwork>, AppError> {
        match app_id {
            Some(id) => self.source.find(id),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageMime;
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeSource {
        result: Option<Artwork>,
        asked: RefCell<Vec<u32>>,
    }

    impl GameArtworkSource for FakeSource {
        fn find(&self, app_id: u32) -> Result<Option<Artwork>, AppError> {
            self.asked.borrow_mut().push(app_id);
            Ok(self.result.clone())
        }
    }

    fn png() -> Artwork {
        Artwork {
            bytes: vec![1, 2, 3],
            mime: ImageMime::Png,
        }
    }

    #[test]
    fn without_app_id_returns_none_and_does_not_call_the_port() {
        let service = ArtworkService::new(FakeSource {
            result: Some(png()),
            ..Default::default()
        });
        assert_eq!(service.artwork(None), Ok(None));
        assert!(service.source.asked.borrow().is_empty());
    }

    #[test]
    fn with_app_id_passes_the_port_result_through() {
        let service = ArtworkService::new(FakeSource {
            result: Some(png()),
            ..Default::default()
        });
        assert_eq!(service.artwork(Some(42)), Ok(Some(png())));
        assert_eq!(*service.source.asked.borrow(), vec![42]);
    }

    #[test]
    fn with_app_id_and_nothing_cached_returns_none() {
        let service = ArtworkService::new(FakeSource::default());
        assert_eq!(service.artwork(Some(7)), Ok(None));
    }

    #[test]
    fn mime_types_are_iana_names() {
        assert_eq!(ImageMime::Png.as_str(), "image/png");
        assert_eq!(ImageMime::Jpeg.as_str(), "image/jpeg");
        assert_eq!(ImageMime::Webp.as_str(), "image/webp");
    }
}
