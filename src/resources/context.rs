use std::borrow::Cow;

use iced::Color;

use super::{
    ColorToken, DimensionToken, IconId, Locale, TextKey, TextSizeToken, resolve_color,
    resolve_dimension, resolve_icon, resolve_text, resolve_text_size,
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

    /// Resolves a localized message, including messages with runtime parameters.
    pub fn text(self, key: TextKey) -> Cow<'static, str> {
        resolve_text(self.locale, key)
    }

    /// Resolves a semantic color for the active application theme.
    pub fn color(self, token: ColorToken) -> Color {
        resolve_color(self.theme, token)
    }

    /// Resolves and scales a semantic layout dimension.
    pub fn dimension(self, token: DimensionToken) -> f32 {
        resolve_dimension(token) * self.layout_scale
    }

    /// Resolves and scales a semantic text size independently from layout dimensions.
    pub fn text_size(self, token: TextSizeToken) -> f32 {
        resolve_text_size(token) * self.text_scale
    }

    /// Resolves the temporary text glyph used for an icon identifier.
    pub fn icon(self, icon: IconId) -> &'static str {
        resolve_icon(icon)
    }
}
