//! Presentation names never replace the backend's stable tool IDs.
use zgpui::native::{Canvas, Rect, Theme, colors::*};

pub fn display_name(id: &str) -> &str {
    match id {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "gemini" => "Gemini CLI",
        "opencode" => "OpenCode",
        "pi" => "Pi",
        "qwen" => "Qwen Code",
        "aider" => "aider",
        "goose" => "goose",
        "zed" => "Zed",
        "vscode" => "Visual Studio Code",
        "cursor" => "Cursor",
        "amp" => "Amp",
        "continue" => "Continue",
        "cline" => "Cline",
        "roo" => "Roo Code",
        "kilo" => "Kilo Code",
        _ => id,
    }
}

pub fn draw(c: &mut Canvas<'_>, id: &str, r: Rect) {
    c.rect(r, r.w * 0.24, BG);
    let inner = r.inset(r.w * 0.16);
    let dark = c.theme == Theme::Dark;
    let color = match id {
        "claude" => {
            if dark {
                0xe8a589ff
            } else {
                0xb95c39ff
            }
        }
        "gemini" | "vscode" => BLUE,
        "qwen" => {
            if dark {
                0xb9a3ffff
            } else {
                0x6845c0ff
            }
        }
        "aider" => GREEN,
        _ => TEXT,
    };
    if let Some(mesh) = crate::agent_logos::get(id) {
        for triangle in mesh.triangles {
            let points = triangle.map(|index| {
                let [x, y] = mesh.points[index as usize];
                (
                    inner.x + x as f32 / 1024. * inner.w,
                    inner.y + y as f32 / 1024. * inner.h,
                )
            });
            c.polygon(&points, color);
        }
    } else {
        // A neutral terminal mark also works for future and non-ASCII IDs.
        let p = |x: f32, y: f32| (inner.x + x * inner.w, inner.y + y * inner.h);
        let stroke = (r.w / 20.).max(1.);
        c.line(p(0.12, 0.23), p(0.42, 0.5), stroke, TEXT);
        c.line(p(0.42, 0.5), p(0.12, 0.77), stroke, TEXT);
        c.line(p(0.54, 0.77), p(0.88, 0.77), stroke, TEXT);
    }
}
