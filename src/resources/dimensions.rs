#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionToken {
    ToggleSize,
    ToggleSpacing,
    TopPaddingVertical,
    TopPaddingHorizontal,
    TopOverlayHeight,
    NavigationButtonPaddingVertical,
    NavigationButtonPaddingHorizontal,
    NavigationSpacing,
    NavigationHorizontalInset,
    ActionButtonPaddingVertical,
    ActionButtonPaddingHorizontal,
    ActionSpacing,
    BottomPaddingTop,
    BottomPaddingHorizontal,
    BottomPaddingBottom,
    MetadataSpacing,
    EdgeButtonRadius,
    StandalonePadding,
}

/// Returns the unscaled logical-pixel value for a semantic layout dimension.
pub fn resolve_dimension(token: DimensionToken) -> f32 {
    match token {
        DimensionToken::MetadataSpacing | DimensionToken::EdgeButtonRadius => 6.0,
        DimensionToken::NavigationButtonPaddingVertical => 8.0,
        DimensionToken::ToggleSpacing | DimensionToken::ActionButtonPaddingVertical => 10.0,
        DimensionToken::ActionSpacing => 12.0,
        DimensionToken::NavigationButtonPaddingHorizontal
        | DimensionToken::NavigationSpacing
        | DimensionToken::ActionButtonPaddingHorizontal => 16.0,
        DimensionToken::ToggleSize => 22.0,
        DimensionToken::TopPaddingVertical
        | DimensionToken::NavigationHorizontalInset
        | DimensionToken::BottomPaddingBottom => 24.0,
        DimensionToken::TopPaddingHorizontal
        | DimensionToken::BottomPaddingHorizontal
        | DimensionToken::StandalonePadding => 32.0,
        DimensionToken::BottomPaddingTop => 48.0,
        DimensionToken::TopOverlayHeight => 104.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Locks the existing control geometry while dimensions move out of the view.
    fn semantic_dimensions_preserve_existing_geometry() {
        assert_eq!(resolve_dimension(DimensionToken::ToggleSize), 22.0);
        assert_eq!(resolve_dimension(DimensionToken::TopOverlayHeight), 104.0);
        assert_eq!(resolve_dimension(DimensionToken::BottomPaddingTop), 48.0);
        assert_eq!(resolve_dimension(DimensionToken::EdgeButtonRadius), 6.0);
    }
}
