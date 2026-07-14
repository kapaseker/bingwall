use std::{borrow::Cow, env};

use super::{TextKey, generated_text_template};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    English,
    SimplifiedChinese,
}

impl Locale {
    /// Detects the active locale from the standard locale environment variables.
    pub fn detect() -> Self {
        let value = env::var("LC_ALL")
            .or_else(|_| env::var("LC_MESSAGES"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default();
        Self::from_name(&value)
    }

    /// Maps a locale name to one of the translations supported by the application.
    pub fn from_name(value: &str) -> Self {
        if value.to_ascii_lowercase().starts_with("zh") {
            Self::SimplifiedChinese
        } else {
            Self::English
        }
    }

    /// Resolves a generated static text key without formatting arguments.
    pub fn text(self, key: TextKey) -> Cow<'static, str> {
        format_template(generated_text_template(self, key), &[])
    }
}

/// Substitutes named values into a generated localized template.
pub(super) fn format_template(
    template: &'static str,
    arguments: &[(&str, String)],
) -> Cow<'static, str> {
    if arguments.is_empty() {
        return Cow::Borrowed(template);
    }

    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let end = after_start
            .find('}')
            .expect("build.rs validates generated text placeholders");
        let name = &after_start[..end];
        let value = arguments
            .iter()
            .find_map(|(argument, value)| (*argument == name).then_some(value))
            .unwrap_or_else(|| panic!("missing value for text placeholder `{name}`"));
        output.push_str(value);
        rest = &after_start[end + 1..];
    }
    output.push_str(rest);
    Cow::Owned(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Verifies only locale names beginning with `zh` select Chinese text.
    fn selects_chinese_only_for_zh_locales() {
        assert_eq!(Locale::from_name("zh_CN.UTF-8"), Locale::SimplifiedChinese);
        assert_eq!(Locale::from_name("en_US.UTF-8"), Locale::English);
    }

    #[test]
    /// Verifies a missing Chinese entry falls back to the default language template.
    fn missing_translation_falls_back_to_default_language() {
        assert_eq!(
            generated_text_template(Locale::SimplifiedChinese, TextKey::page_counter),
            "{current} / {total}"
        );
    }

    #[test]
    /// Formats generated placeholders with values supplied by the resource macro.
    fn formats_generated_text_arguments() {
        assert_eq!(
            format_template(
                "{current} / {total}",
                &[("current", "3".into()), ("total", "10".into())]
            ),
            "3 / 10"
        );
    }
}
