use iced::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorResource {
    rgba: [f32; 4],
}

impl ColorResource {
    /// Creates a compile-time color descriptor from normalized RGBA channels.
    pub(crate) const fn new(rgba: [f32; 4]) -> Self {
        Self { rgba }
    }

    /// Resolves this descriptor into an Iced color.
    pub(crate) fn resolve(self) -> Color {
        let [red, green, blue, alpha] = self.rgba;
        Color::from_rgba(red, green, blue, alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::generated_colors;

    #[test]
    /// Locks generated dark overlay descriptors to their properties source values.
    fn generated_palette_preserves_overlay_colors() {
        assert_eq!(
            generated_colors::surface_fallback.resolve(),
            Color::from_rgb8(18, 18, 18)
        );
        assert_eq!(
            generated_colors::interactive.resolve(),
            Color::from_rgba8(0, 0, 0, 0.52)
        );
        assert_eq!(
            generated_colors::interactive_hovered.resolve(),
            Color::from_rgba8(0, 0, 0, 0.72)
        );
        assert_eq!(
            generated_colors::interactive_disabled.resolve(),
            Color::from_rgba8(0, 0, 0, 0.18)
        );
    }
}
