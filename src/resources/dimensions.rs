use super::{DimensionKey, generated_dimension};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionScale {
    Layout,
    Text,
}

/// Resolves a generated dimension with its declared independent scale factor.
pub(super) fn resolve_dimension(key: DimensionKey, layout_scale: f32, text_scale: f32) -> f32 {
    let (value, scale) = generated_dimension(key);
    value
        * match scale {
            DimensionScale::Layout => layout_scale,
            DimensionScale::Text => text_scale,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Verifies generated dimensions select layout and text scaling independently.
    fn generated_dimensions_use_declared_scale_kind() {
        assert_eq!(resolve_dimension(DimensionKey::toggle_size, 2.0, 3.0), 44.0);
        assert_eq!(resolve_dimension(DimensionKey::text_label, 2.0, 3.0), 48.0);
    }
}
