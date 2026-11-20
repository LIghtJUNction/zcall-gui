#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::{fs, path::PathBuf};
use zgpui::native::Theme;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}
impl ThemeChoice {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}
pub struct Preferences {
    pub theme: ThemeChoice,
    pub language: &'static str,
    pub core_binary: Option<String>,
    system_dark: bool,
    path: PathBuf,
}
impl Preferences {
    #[cfg(test)]
    pub fn test_defaults() -> Self {
        Self {
            theme: ThemeChoice::Light,
            language: "en",
            core_binary: None,
            system_dark: false,
            path: PathBuf::new(),
        }
    }
    pub fn load() -> Self {
        let path = config_dir().join("settings.conf");
        let mut prefs = Self {
            theme: ThemeChoice::System,
            language: "system",
            core_binary: None,
            system_dark: false,
            path,
        };
        if let Ok(text) = fs::read_to_string(&prefs.path) {
            for line in text.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    match key {
                        "theme" => {
                            if let Some(theme) = ThemeChoice::parse(value) {
                                prefs.theme = theme;
                            }
                        }
                        "language" => {
                            if let Some(language) =
                                crate::i18n::LANGUAGES.iter().find(|id| **id == value)
                            {
                                prefs.language = language;
                            }
                        }
                        "core_binary" if valid_binary(value) => {
                            prefs.core_binary = Some(value.into())
                        }
                        _ => {}
                    }
                }
            }
        }
        prefs.apply();
        prefs
    }
    pub fn apply(&mut self) {
        self.system_dark = system_dark();
        crate::i18n::select(self.language);
    }
    pub fn resolved_theme(&self) -> Theme {
        if self.theme == ThemeChoice::Dark
            || (self.theme == ThemeChoice::System && self.system_dark)
        {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
    pub fn save(&mut self) -> Result<(), String> {
        self.apply();
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let binary = self.core_binary.as_deref().unwrap_or("");
        if !binary.is_empty() && !valid_binary(binary) {
            return Err("The remembered gateway path is invalid.".into());
        }
        fs::write(
            &self.path,
            format!(
                "theme={}\nlanguage={}\ncore_binary={}\n",
                self.theme.id(),
                self.language,
                binary
            ),
        )
        .map_err(|e| e.to_string())
    }
}
fn valid_binary(value: &str) -> bool {
    value.len() <= 4096
        && !value.chars().any(char::is_control)
        && std::path::Path::new(value).is_absolute()
}
fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(base) = std::env::var_os("APPDATA") {
        return PathBuf::from(base).join("zcall-gui");
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Application Support/zcall-gui");
    }
    if let Some(base) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(base).join("zcall-gui");
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/zcall-gui")
}
fn system_dark() -> bool {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "advapi32")]
        unsafe extern "system" {
            fn RegGetValueW(
                key: *mut std::ffi::c_void,
                subkey: *const u16,
                value: *const u16,
                flags: u32,
                kind: *mut u32,
                data: *mut std::ffi::c_void,
                size: *mut u32,
            ) -> i32;
        }
        let key = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        let name = "AppsUseLightTheme\0".encode_utf16().collect::<Vec<_>>();
        let (mut light, mut size) = (1u32, 4u32);
        let status = unsafe {
            RegGetValueW(
                -2147483647isize as *mut _,
                key.as_ptr(),
                name.as_ptr(),
                0x10,
                std::ptr::null_mut(),
                (&mut light as *mut u32).cast(),
                &mut size,
            )
        };
        status == 0 && light == 0
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .is_ok_and(|o| {
                o.status.success()
                    && String::from_utf8_lossy(&o.stdout)
                        .to_ascii_lowercase()
                        .contains("dark")
            })
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(theme) = std::env::var("GTK_THEME") {
            return theme.to_ascii_lowercase().contains("dark");
        }
        if let Ok(output) = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
            && output.status.success()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.contains("prefer-dark") {
                return true;
            }
            if text.contains("prefer-light") {
                return false;
            }
        }
        if let Some(home) = std::env::var_os("HOME") {
            let path = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(home).join(".config"));
            for version in ["gtk-4.0", "gtk-3.0"] {
                if let Ok(text) = fs::read_to_string(path.join(version).join("settings.ini"))
                    && text.lines().any(|line| {
                        line.trim() == "gtk-application-prefer-dark-theme=1"
                            || line.trim() == "gtk-application-prefer-dark-theme=true"
                    })
                {
                    return true;
                }
            }
        }
        false
    }
}
