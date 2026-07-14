#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSizeToken {
    Status,
    Counter,
    Label,
    Loading,
    Standalone,
    Description,
    NavigationIcon,
}

/// Returns the unscaled logical-pixel value for a semantic text size.
pub fn resolve_text_size(token: TextSizeToken) -> f32 {
    match token {
        TextSizeToken::Status | TextSizeToken::Counter => 14.0,
        TextSizeToken::Label | TextSizeToken::Loading => 16.0,
        TextSizeToken::Standalone => 18.0,
        TextSizeToken::Description => 20.0,
        TextSizeToken::NavigationIcon => 38.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Locks the existing type scale while text sizes move out of the view.
    fn semantic_text_sizes_preserve_existing_type_scale() {
        assert_eq!(resolve_text_size(TextSizeToken::Status), 14.0);
        assert_eq!(resolve_text_size(TextSizeToken::Label), 16.0);
        assert_eq!(resolve_text_size(TextSizeToken::Standalone), 18.0);
        assert_eq!(resolve_text_size(TextSizeToken::Description), 20.0);
        assert_eq!(resolve_text_size(TextSizeToken::NavigationIcon), 38.0);
    }
}
