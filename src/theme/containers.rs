use std::f32::consts::PI;

use iced::gradient;
use iced::widget::container;
use iced::{Background, Theme};

use crate::resources::{ColorToken, ResourceContext};

/// Paints a dark fallback behind loading and unsupported states.
pub(crate) fn fallback_background(resources: ResourceContext, _theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(resources.color(ColorToken::TextOnImage)),
        background: Some(Background::Color(
            resources.color(ColorToken::SurfaceFallback),
        )),
        ..container::Style::default()
    }
}

/// Paints a top-to-transparent scrim behind the daily-change setting.
pub(crate) fn top_scrim(resources: ResourceContext, _theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(resources.color(ColorToken::TextOnImage)),
        background: Some(
            gradient::Linear::new(PI)
                .add_stop(0.0, resources.color(ColorToken::SurfaceScrimMedium))
                .add_stop(1.0, resources.color(ColorToken::SurfaceScrimTransparent))
                .into(),
        ),
        ..container::Style::default()
    }
}

/// Paints a transparent-to-bottom scrim behind metadata and actions.
pub(crate) fn bottom_scrim(resources: ResourceContext, _theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(resources.color(ColorToken::TextOnImage)),
        background: Some(
            gradient::Linear::new(PI)
                .add_stop(0.0, resources.color(ColorToken::SurfaceScrimTransparent))
                .add_stop(0.45, resources.color(ColorToken::SurfaceScrimWeak))
                .add_stop(1.0, resources.color(ColorToken::SurfaceScrimStrong))
                .into(),
        ),
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use iced::Color;

    use super::*;
    use crate::resources::{AppTheme, Locale};

    #[test]
    /// Verifies fallback surfaces retain the existing dark appearance.
    fn fallback_style_uses_semantic_surface_colors() {
        let resources = ResourceContext::new(Locale::English, AppTheme::Dark, 1.0, 1.0);
        let style = fallback_background(resources, &Theme::Dark);

        assert_eq!(style.text_color, Some(Color::WHITE));
        assert_eq!(
            style.background,
            Some(Background::Color(Color::from_rgb8(18, 18, 18)))
        );
    }
}
