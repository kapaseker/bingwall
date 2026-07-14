use super::{ImageKey, generated_image};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageResource {
    Placeholder(&'static str),
    File {
        path: &'static str,
        bytes: &'static [u8],
    },
}

impl ImageResource {
    /// Returns a temporary glyph and fails loudly if a real file is wired as text.
    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Placeholder(value) => value,
            Self::File { path, .. } => {
                panic!("image resource `{path}` is not a text placeholder")
            }
        }
    }
}

/// Resolves a generated image key to its configured source declaration.
pub(super) fn resolve_image(key: ImageKey) -> ImageResource {
    generated_image(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Keeps navigation images as configured placeholders until assets are supplied.
    fn navigation_images_use_configured_placeholders() {
        assert_eq!(resolve_image(ImageKey::previous).placeholder(), "‹");
        assert_eq!(resolve_image(ImageKey::next).placeholder(), "›");
    }
}
