use iced::widget::button;
use iced::{Background, Theme};

/// Uses Iced's primary button treatment for the main wallpaper action.
pub(crate) fn primary_action(theme: &Theme, status: button::Status) -> button::Style {
    button::primary(theme, status)
}

/// Uses Iced's secondary button treatment for the refresh action.
pub(crate) fn secondary_action(theme: &Theme, status: button::Status) -> button::Style {
    button::secondary(theme, status)
}

/// Paints compact translucent navigation buttons over the wallpaper image.
pub(crate) fn edge_navigation(theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => color!(interactive_hovered),
        button::Status::Disabled => color!(interactive_disabled),
        button::Status::Active | button::Status::Pressed => color!(interactive),
    };
    let mut style = button::secondary(theme, status);
    style.text_color = color!(text_on_image);
    style.background = Some(Background::Color(background));
    style.border.radius = dimension!(edge_button_radius).into();
    style
}

#[cfg(test)]
mod tests {
    use iced::Color;

    use super::*;
    #[test]
    /// Verifies navigation states retain their configured overlay opacity.
    fn navigation_style_uses_generated_state_colors() {
        let theme = Theme::Dark;
        let active = edge_navigation(&theme, button::Status::Active);
        let hovered = edge_navigation(&theme, button::Status::Hovered);
        let disabled = edge_navigation(&theme, button::Status::Disabled);

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
