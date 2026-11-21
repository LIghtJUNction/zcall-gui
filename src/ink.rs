//! Quiet neutral surfaces and semantic accents for the native control panel.
//! Use the toolkit's roles so buttons, focus, inputs and text share one palette.
use zgpui::native::{Palette, Theme};

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            background: 0x14161aff,
            surface: 0x1c1f24ff,
            text: 0xe8eaedff,
            muted: 0x9aa0aaff,
            line: 0x30343bff,
            accent: 0x8ab4f8ff,
            accent_hover: 0xa8c7faff,
            accent_soft: 0x25334aff,
            hover: 0x282c33ff,
            success: 0x81c995ff,
            success_soft: 0x203b2bff,
            danger: 0xf28b82ff,
            danger_soft: 0x452a2aff,
            disabled_surface: 0x252930ff,
            disabled_text: 0x777e89ff,
            on_accent: 0x102343ff,
        },
        Theme::Light => Palette {
            background: 0xf7f8faff,
            surface: 0xffffffff,
            text: 0x202124ff,
            muted: 0x646b76ff,
            line: 0xe3e6ebff,
            accent: 0x1a73e8ff,
            accent_hover: 0x1765ccff,
            accent_soft: 0xe8f0feff,
            hover: 0xf1f3f6ff,
            success: 0x188038ff,
            success_soft: 0xe6f4eaff,
            danger: 0xc5221fff,
            danger_soft: 0xfce8e6ff,
            disabled_surface: 0xf1f3f6ff,
            disabled_text: 0x8b919bff,
            on_accent: 0xffffffff,
        },
    }
}
