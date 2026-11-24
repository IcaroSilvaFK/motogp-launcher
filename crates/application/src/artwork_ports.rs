use crate::AppError;

/// Image format of a piece of artwork, detected from its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMime {
    Png,
    Jpeg,
    Webp,
}

impl ImageMime {
    /// The IANA media type, e.g. `image/png`.
    pub fn as_str(self) -> &'static str {
        match self {
            ImageMime::Png => "image/png",
            ImageMime::Jpeg => "image/jpeg",
            ImageMime::Webp => "image/webp",
        }
    }
}

/// A game image read from a local cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artwork {
    pub bytes: Vec<u8>,
    pub mime: ImageMime,
}

/// Port: read-only source of the artwork of a Steam game. Never downloads.
pub trait GameArtworkSource {
    /// The best cached image for the App ID; `None` when nothing is cached.
    fn find(&self, app_id: u32) -> Result<Option<Artwork>, AppError>;
}
