use std::borrow::Cow;

use iced::Color;

use super::{
    ColorKey, DimensionKey, ImageKey, ImageResource, Locale, TextKey, colors::resolve_color,
    dimensions::resolve_dimension, generated_text_template, icons::resolve_image,
    strings::format_template,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTheme {
    Dark,
}

#[derive(Debug, Clone, Copy)]
pub struct ResourceContext {
    locale: Locale,
    theme: AppTheme,
    layout_scale: f32,
    text_scale: f32,
}

impl ResourceContext {
    /// Creates a resource resolver for the current locale, theme, and scale factors.
    pub fn new(locale: Locale, theme: AppTheme, layout_scale: f32, text_scale: f32) -> Self {
        Self {
            locale,
            theme,
            layout_scale,
            text_scale,
        }
    }

    /// Resolves a generated localized text key and substitutes its named arguments.
    pub fn text(self, key: TextKey, arguments: &[(&str, String)]) -> Cow<'static, str> {
        format_template(generated_text_template(self.locale, key), arguments)
    }

    /// Resolves a generated semantic color key for the active application theme.
    pub fn color(self, key: ColorKey) -> Color {
        resolve_color(self.theme, key)
    }

    /// Resolves a generated dimension key using its declared layout or text scale.
    pub fn dimension(self, key: DimensionKey) -> f32 {
        resolve_dimension(key, self.layout_scale, self.text_scale)
    }

    /// Resolves a generated image key to a placeholder or packaged file declaration.
    pub fn image(self, key: ImageKey) -> ImageResource {
        resolve_image(key)
    }
}
