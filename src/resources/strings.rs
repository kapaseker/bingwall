use std::{
    borrow::Cow,
    env,
    sync::{LazyLock, RwLock},
};

#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locale {
    English,
    SimplifiedChinese,
}

#[derive(Debug)]
struct LocaleStore {
    locale: RwLock<Locale>,
}

impl LocaleStore {
    /// Creates an independently owned locale store.
    fn new(locale: Locale) -> Self {
        Self {
            locale: RwLock::new(locale),
        }
    }

    /// Returns the current locale while recovering a poisoned read lock.
    fn current(&self) -> Locale {
        *self
            .locale
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Replaces the current locale while recovering a poisoned write lock.
    fn set(&self, locale: Locale) {
        *self
            .locale
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = locale;
    }
}

static CURRENT_LOCALE: LazyLock<LocaleStore> = LazyLock::new(|| LocaleStore::new(Locale::detect()));

#[cfg(test)]
static LOCALE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
/// Serializes tests that mutate the application-wide locale.
pub(crate) fn lock_locale_tests() -> MutexGuard<'static, ()> {
    LOCALE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Returns the application-wide locale used by generated text resources.
pub fn current_locale() -> Locale {
    CURRENT_LOCALE.current()
}

/// Changes the application-wide locale used by generated text resources.
pub fn set_locale(locale: Locale) {
    CURRENT_LOCALE.set(locale);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextResource {
    default: &'static str,
    simplified_chinese: Option<&'static str>,
}

impl TextResource {
    /// Creates a compile-time localized text descriptor with an optional Chinese override.
    pub(crate) const fn new(
        default: &'static str,
        simplified_chinese: Option<&'static str>,
    ) -> Self {
        Self {
            default,
            simplified_chinese,
        }
    }

    /// Resolves this descriptor using the application-wide locale.
    pub(crate) fn resolve(self, arguments: &[(&str, String)]) -> Cow<'static, str> {
        resolve_text(current_locale(), self, arguments)
    }

    /// Selects the localized template and falls back to the default when absent.
    fn template(self, locale: Locale) -> &'static str {
        match locale {
            Locale::English => self.default,
            Locale::SimplifiedChinese => self.simplified_chinese.unwrap_or(self.default),
        }
    }
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
}

/// Resolves and formats a generated text descriptor for an explicit locale.
fn resolve_text(
    locale: Locale,
    resource: TextResource,
    arguments: &[(&str, String)],
) -> Cow<'static, str> {
    format_template(resource.template(locale), arguments)
}

/// Substitutes named values into a generated localized template.
fn format_template(template: &'static str, arguments: &[(&str, String)]) -> Cow<'static, str> {
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
    use crate::resources::generated_text;

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
            generated_text::page_counter.template(Locale::SimplifiedChinese),
            "{current} / {total}"
        );
    }

    #[test]
    /// Formats generated placeholders with values supplied by the resource macro.
    fn formats_generated_text_arguments() {
        assert_eq!(
            resolve_text(
                Locale::English,
                generated_text::page_counter,
                &[("current", "3".into()), ("total", "10".into())]
            ),
            "3 / 10"
        );
    }

    #[test]
    /// Verifies an isolated locale store supports runtime language changes.
    fn locale_store_switches_language_at_runtime() {
        let store = LocaleStore::new(Locale::English);
        assert_eq!(store.current(), Locale::English);

        store.set(Locale::SimplifiedChinese);
        assert_eq!(store.current(), Locale::SimplifiedChinese);
    }
}
