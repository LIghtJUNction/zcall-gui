//! Application-local Ink colors, derived from the oh-my-ztty terminal preset.
//! Use the toolkit's roles so buttons, focus, inputs and text share one palette.
use zgpui::native::{Palette, Theme};

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            background: 0x161412ff,
            surface: 0x201c18ff,
            text: 0xe8ded2ff,
            muted: 0xa3978bff,
            line: 0x383029ff,
            accent: 0xd4a373ff,
            accent_hover: 0xe2b88eff,
            accent_soft: 0x45382eff,
            hover: 0x2b241fff,
            success: 0xd4a373ff,
            success_soft: 0x45382eff,
            danger: 0xe59a91ff,
            danger_soft: 0x3c2723ff,
            disabled_surface: 0x2b241fff,
            disabled_text: 0x82766aff,
            on_accent: 0x161412ff,
        },
        Theme::Light => Palette {
            background: 0xf5f0e8ff,
            surface: 0xece4d9ff,
            text: 0x322a24ff,
            muted: 0x74685eff,
            line: 0xd8cabbff,
            accent: 0x885527ff,
            accent_hover: 0x704018ff,
            accent_soft: 0xe8d5bdff,
            hover: 0xe8ddd0ff,
            success: 0x885527ff,
            success_soft: 0xe8d5bdff,
            danger: 0xa34135ff,
            danger_soft: 0xfff0edff,
            disabled_surface: 0xe8ddd0ff,
            disabled_text: 0x82766aff,
            on_accent: 0xfff7efff,
        },
    }
}
