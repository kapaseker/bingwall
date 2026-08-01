#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageResource {
    path: &'static str,
    bytes: &'static [u8],
}

impl ImageResource {
    /// Creates a compile-time descriptor for an embedded image file.
    pub(crate) const fn new(path: &'static str, bytes: &'static [u8]) -> Self {
        Self { path, bytes }
    }

    /// Returns the resource path relative to the assets resource directory.
    #[cfg(test)]
    pub(crate) fn path(self) -> &'static str {
        self.path
    }

    /// Creates an Iced SVG handle from an embedded file resource.
    pub fn svg_handle(self) -> iced::widget::svg::Handle {
        iced::widget::svg::Handle::from_memory(self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::generated_images;

    #[test]
    /// Verifies navigation icons are embedded from their configured SVG files.
    fn navigation_images_use_embedded_svg_files() {
        assert_eq!(generated_images::ic_left.path(), "images/ic_left.svg");
        assert_eq!(generated_images::ic_right.path(), "images/ic_right.svg");
    }
}
