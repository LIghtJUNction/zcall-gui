//! Window-local color roles shared by the console and native widgets.
use zgpui::native::{Palette, Theme};

pub use zgpui::native::colors::{BLUE as ACCENT, BLUE_PALE as ACCENT_SOFT, GREEN as SUCCESS};

pub const COLOR_LABELS: [&str; 7] = [
    "settings.color_background",
    "settings.color_surface",
    "settings.color_text",
    "settings.color_muted",
    "settings.color_accent",
    "settings.color_success",
    "settings.color_danger",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors([u32; COLOR_LABELS.len()]);

impl Colors {
    pub fn defaults(theme: Theme) -> Self {
        Self(match theme {
            Theme::Dark => [
                0x191612ff, 0x221e19ff, 0xf5ece2ff, 0xbcae9fff, 0xe4ad78ff, 0xe4bb71ff, 0xf0a39aff,
            ],
            Theme::Light => [
                0xf8f3ecff, 0xfffcf7ff, 0x30251eff, 0x6e5e52ff, 0x994f2cff, 0x80551aff, 0xa52e27ff,
            ],
        })
    }

    pub fn parse(values: impl IntoIterator<Item = impl AsRef<str>>) -> Option<Self> {
        let mut values = values.into_iter();
        let mut colors = [0; COLOR_LABELS.len()];
        for color in &mut colors {
            let value = values.next()?;
            let digits = value.as_ref().trim();
            let digits = digits.strip_prefix('#').unwrap_or(digits);
            if digits.len() != 6 || !digits.bytes().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            *color = (u32::from_str_radix(digits, 16).ok()? << 8) | 255;
        }
        values.next().is_none().then_some(Self(colors))
    }

    pub fn fields(self) -> [String; COLOR_LABELS.len()] {
        self.0.map(|color| format!("#{:06X}", color >> 8))
    }

    pub fn palette(self) -> Palette {
        let [background, surface, text, muted, accent, success, danger] = self.0;
        let luminance: f64 = [(24, 0.2126), (16, 0.7152), (8, 0.0722)]
            .into_iter()
            .map(|(shift, weight)| {
                let channel = ((accent >> shift) & 255) as f64 / 255.;
                let linear = if channel <= 0.04045 {
                    channel / 12.92
                } else {
                    ((channel + 0.055) / 1.055).powf(2.4)
                };
                weight * linear
            })
            .sum();
        Palette {
            background,
            surface,
            text,
            muted,
            line: mix(surface, text, 16),
            accent,
            accent_hover: mix(accent, text, 16),
            accent_soft: mix(surface, accent, 18),
            hover: mix(surface, text, 7),
            success,
            success_soft: mix(surface, success, 16),
            danger,
            danger_soft: mix(surface, danger, 16),
            disabled_surface: mix(surface, text, 7),
            disabled_text: mix(muted, surface, 25),
            on_accent: if luminance > 0.179 {
                0x000000ff
            } else {
                0xffffffff
            },
        }
    }
}

fn mix(base: u32, color: u32, percent: u32) -> u32 {
    [24, 16, 8].into_iter().fold(255, |rgba, shift| {
        let channel =
            ((base >> shift & 255) * (100 - percent) + (color >> shift & 255) * percent + 50) / 100;
        rgba | (channel << shift)
    })
}

pub struct Editor {
    pub theme: Theme,
    pub preview: Colors,
    pub fields: [String; COLOR_LABELS.len()],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_round_trip_and_select_a_contrasting_button_label() {
        for theme in [Theme::Light, Theme::Dark] {
            let defaults = Colors::defaults(theme);
            assert_eq!(Colors::parse(defaults.fields()), Some(defaults));
        }
        assert_eq!(
            Colors::defaults(Theme::Light).palette().on_accent,
            0xffffffff
        );
        assert_eq!(
            Colors::defaults(Theme::Dark).palette().on_accent,
            0x000000ff
        );
        assert!(Colors::parse(["#000000"; 6]).is_none());
        assert!(Colors::parse(["+00000"; 7]).is_none());
        assert!(Colors::parse(["#FFFFFF"; 8]).is_none());
    }
}
