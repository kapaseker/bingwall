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

#[cfg(test)]
mod tests {
    use crate::resources::generated_images;

    #[test]
    /// Keeps navigation images as generated placeholders until assets are supplied.
    fn navigation_images_use_configured_placeholders() {
        assert_eq!(generated_images::previous.placeholder(), "‹");
        assert_eq!(generated_images::next.placeholder(), "›");
    }
}
