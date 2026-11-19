use std::sync::atomic::{AtomicU8, Ordering};
static ACTIVE: AtomicU8 = AtomicU8::new(0);
pub const LANGUAGES: [&str; 5] = ["system", "zh-CN", "zh-TW", "en", "ja"];
pub const LANGUAGE_NAMES: [&str; 4] = ["简体中文", "繁體中文", "English", "日本語"];
const CATALOGS: [&str; 4] = [
    include_str!("../resources/locales/zh-CN.txt"),
    include_str!("../resources/locales/zh-TW.txt"),
    include_str!("../resources/locales/en.txt"),
    include_str!("../resources/locales/ja.txt"),
];
pub fn locale_index(locale: &str) -> u8 {
    let locale = locale.to_ascii_lowercase().replace('_', "-");
    if locale.starts_with("zh-tw") || locale.starts_with("zh-hk") || locale.contains("hant") {
        1
    } else if locale.starts_with("zh") {
        0
    } else if locale.starts_with("ja") {
        3
    } else {
        2
    }
}
pub fn system_locale() -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key)
            && !value.is_empty()
        {
            return value;
        }
    }
    #[cfg(target_os = "windows")]
    {
        unsafe extern "system" {
            fn GetUserDefaultLocaleName(buffer: *mut u16, capacity: i32) -> i32;
        }
        let mut text = [0u16; 85];
        let n = unsafe { GetUserDefaultLocaleName(text.as_mut_ptr(), text.len() as i32) };
        if n > 1 {
            return String::from_utf16_lossy(&text[..n as usize - 1]);
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
            && output.status.success()
        {
            return String::from_utf8_lossy(&output.stdout).trim().into();
        }
    }
    "en".into()
}
pub fn select(language: &str) {
    let system = system_locale();
    ACTIVE.store(
        locale_index(if language == "system" {
            &system
        } else {
            language
        }),
        Ordering::Relaxed,
    );
}

pub fn tr(key: &'static str) -> &'static str {
    CATALOGS[ACTIVE.load(Ordering::Relaxed) as usize]
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find_map(|(id, text)| (id == key).then_some(text))
        .unwrap_or(key)
}
pub fn message(key: &'static str, values: &[(&str, &str)]) -> String {
    let mut text = tr(key).to_owned();
    for (key, value) in values {
        text = text.replace(&format!("{{{key}}}"), value);
    }
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn catalogs_have_same_complete_keys() {
        let expected = CATALOGS[0]
            .lines()
            .map(|l| l.split_once('\t').unwrap().0)
            .collect::<BTreeSet<_>>();
        assert!(expected.len() > 150);
        for catalog in CATALOGS {
            let rows = catalog
                .lines()
                .map(|l| l.split_once('\t').unwrap())
                .collect::<Vec<_>>();
            assert!(rows.iter().all(|(_, text)| !text.trim().is_empty()));
            let keys = rows.iter().map(|(key, _)| *key).collect::<BTreeSet<_>>();
            assert_eq!(keys.len(), rows.len());
            assert_eq!(keys, expected);
        }
    }
    #[test]
    fn locales_match_expected_fallbacks() {
        assert_eq!(locale_index("zh_HK.UTF-8"), 1);
        assert_eq!(locale_index("zh-Hant"), 1);
        assert_eq!(locale_index("zh_CN.UTF-8"), 0);
        assert_eq!(locale_index("ja_JP.UTF-8"), 3);
        assert_eq!(locale_index("fr_FR"), 2);
    }
}
