use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    English,
    SimplifiedChinese,
}

impl Locale {
    pub fn detect() -> Self {
        let value = env::var("LC_ALL")
            .or_else(|_| env::var("LC_MESSAGES"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default();
        Self::from_name(&value)
    }

    pub fn from_name(value: &str) -> Self {
        if value.to_ascii_lowercase().starts_with("zh") {
            Self::SimplifiedChinese
        } else {
            Self::English
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_chinese_only_for_zh_locales() {
        assert_eq!(Locale::from_name("zh_CN.UTF-8"), Locale::SimplifiedChinese);
        assert_eq!(Locale::from_name("en_US.UTF-8"), Locale::English);
    }
}
