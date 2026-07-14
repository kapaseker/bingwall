#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconId {
    Previous,
    Next,
}

/// Resolves an icon identifier to its temporary text placeholder.
pub fn resolve_icon(icon: IconId) -> &'static str {
    match icon {
        IconId::Previous => "‹",
        IconId::Next => "›",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Keeps navigation glyphs as placeholders until real icon assets are available.
    fn navigation_icons_use_existing_placeholders() {
        assert_eq!(resolve_icon(IconId::Previous), "‹");
        assert_eq!(resolve_icon(IconId::Next), "›");
    }
}
