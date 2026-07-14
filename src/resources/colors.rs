use iced::Color;

use super::AppTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorToken {
    TextOnImage,
    SurfaceFallback,
    SurfaceScrimTransparent,
    SurfaceScrimWeak,
    SurfaceScrimMedium,
    SurfaceScrimStrong,
    Interactive,
    InteractiveHovered,
    InteractiveDisabled,
}

/// Resolves a semantic color token for the selected application theme.
pub fn resolve_color(theme: AppTheme, token: ColorToken) -> Color {
    match (theme, token) {
        (AppTheme::Dark, ColorToken::TextOnImage) => Color::WHITE,
        (AppTheme::Dark, ColorToken::SurfaceFallback) => Color::from_rgb8(18, 18, 18),
        (AppTheme::Dark, ColorToken::SurfaceScrimTransparent) => Color::TRANSPARENT,
        (AppTheme::Dark, ColorToken::SurfaceScrimWeak) => Color::from_rgba8(0, 0, 0, 0.42),
        (AppTheme::Dark, ColorToken::SurfaceScrimMedium) => Color::from_rgba8(0, 0, 0, 0.72),
        (AppTheme::Dark, ColorToken::SurfaceScrimStrong) => Color::from_rgba8(0, 0, 0, 0.82),
        (AppTheme::Dark, ColorToken::Interactive) => Color::from_rgba8(0, 0, 0, 0.52),
        (AppTheme::Dark, ColorToken::InteractiveHovered) => Color::from_rgba8(0, 0, 0, 0.72),
        (AppTheme::Dark, ColorToken::InteractiveDisabled) => Color::from_rgba8(0, 0, 0, 0.18),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Locks the existing dark overlay palette while UI styles are extracted.
    fn dark_palette_preserves_existing_overlay_colors() {
        assert_eq!(
            resolve_color(AppTheme::Dark, ColorToken::SurfaceFallback),
            Color::from_rgb8(18, 18, 18)
        );
        assert_eq!(
            resolve_color(AppTheme::Dark, ColorToken::Interactive),
            Color::from_rgba8(0, 0, 0, 0.52)
        );
        assert_eq!(
            resolve_color(AppTheme::Dark, ColorToken::InteractiveHovered),
            Color::from_rgba8(0, 0, 0, 0.72)
        );
        assert_eq!(
            resolve_color(AppTheme::Dark, ColorToken::InteractiveDisabled),
            Color::from_rgba8(0, 0, 0, 0.18)
        );
    }
}
