use iced::widget::button;
use iced::{Background, Theme};

use crate::resources::{ColorToken, DimensionToken, ResourceContext};

/// Uses Iced's primary button treatment for the main wallpaper action.
pub(crate) fn primary_action(theme: &Theme, status: button::Status) -> button::Style {
    button::primary(theme, status)
}

/// Uses Iced's secondary button treatment for the refresh action.
pub(crate) fn secondary_action(theme: &Theme, status: button::Status) -> button::Style {
    button::secondary(theme, status)
}

/// Paints compact translucent navigation buttons over the wallpaper image.
pub(crate) fn edge_navigation(
    resources: ResourceContext,
    theme: &Theme,
    status: button::Status,
) -> button::Style {
    let background = match status {
        button::Status::Hovered => ColorToken::InteractiveHovered,
        button::Status::Disabled => ColorToken::InteractiveDisabled,
        button::Status::Active | button::Status::Pressed => ColorToken::Interactive,
    };
    let mut style = button::secondary(theme, status);
    style.text_color = resources.color(ColorToken::TextOnImage);
    style.background = Some(Background::Color(resources.color(background)));
    style.border.radius = resources.dimension(DimensionToken::EdgeButtonRadius).into();
    style
}

#[cfg(test)]
mod tests {
    use iced::Color;

    use super::*;
    use crate::resources::{AppTheme, Locale};

    /// Creates an unscaled dark resource context for style assertions.
    fn resources() -> ResourceContext {
        ResourceContext::new(Locale::English, AppTheme::Dark, 1.0, 1.0)
    }

    #[test]
    /// Verifies navigation states retain their existing overlay opacity.
    fn navigation_style_uses_semantic_state_colors() {
        let theme = Theme::Dark;
        let active = edge_navigation(resources(), &theme, button::Status::Active);
        let hovered = edge_navigation(resources(), &theme, button::Status::Hovered);
        let disabled = edge_navigation(resources(), &theme, button::Status::Disabled);

        assert_eq!(active.text_color, Color::WHITE);
        assert_eq!(
            active.background,
            Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.52)))
        );
        assert_eq!(
            hovered.background,
            Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.72)))
        );
        assert_eq!(
            disabled.background,
            Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.18)))
        );
        assert_eq!(active.border.radius, 6.0.into());
    }
}
