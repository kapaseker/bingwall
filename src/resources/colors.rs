use iced::Color;

use super::{AppTheme, ColorKey, generated_color};

/// Resolves a generated semantic color for the selected application theme.
pub(super) fn resolve_color(_theme: AppTheme, key: ColorKey) -> Color {
    let [red, green, blue, alpha] = generated_color(key);
    Color::from_rgba(red, green, blue, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Locks the generated dark overlay palette to the properties source values.
    fn generated_palette_preserves_overlay_colors() {
        assert_eq!(
            resolve_color(AppTheme::Dark, ColorKey::surface_fallback),
            Color::from_rgb8(18, 18, 18)
        );
        assert_eq!(
            resolve_color(AppTheme::Dark, ColorKey::interactive),
            Color::from_rgba8(0, 0, 0, 0.52)
        );
        assert_eq!(
            resolve_color(AppTheme::Dark, ColorKey::interactive_hovered),
            Color::from_rgba8(0, 0, 0, 0.72)
        );
        assert_eq!(
            resolve_color(AppTheme::Dark, ColorKey::interactive_disabled),
            Color::from_rgba8(0, 0, 0, 0.18)
        );
    }
}
