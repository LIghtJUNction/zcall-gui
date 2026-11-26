//! Native, local-only agent configuration editing.
use super::*;

const LOAD: &str = "agent-config-load";
const CHECK: &str = "agent-config-check";
const SAVE: &str = "agent-config-save";
const LIMIT: usize = 256 * 1024;
const FONT: i32 = 14;
const LINE_HEIGHT: f32 = 22.;

pub(super) fn handles(label: &str) -> bool {
    matches!(label, LOAD | CHECK | SAVE)
}

fn guide(agent: &str, file: &str) -> (&'static str, &'static [&'static str]) {
    match agent {
        "codex" => (
            "https://learn.chatgpt.com/docs/config-file/config-reference",
            &[
                "model",
                "model_provider",
                "[model_providers.zcalld]",
                "base_url",
                "wire_api",
                "env_key",
            ],
        ),
        "claude" => (
            "https://code.claude.com/docs/en/settings",
            &[
                "env",
                "ANTHROPIC_BASE_URL",
                "ANTHROPIC_MODEL",
                "permissions",
            ],
        ),
        "gemini" if file.ends_with(".env") => (
            "https://geminicli.com/docs/reference/configuration/",
            &["GOOGLE_GEMINI_BASE_URL", "GEMINI_API_KEY"],
        ),
        "gemini" => (
            "https://geminicli.com/docs/reference/configuration/",
            &["model", "security", "selectedType", "tools"],
        ),
        "opencode" => (
            "https://opencode.ai/docs/config/",
            &["provider", "model", "baseURL", "models"],
        ),
        "pi" if file.ends_with("models.json") => (
            "https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/models.md",
            &["providers", "baseUrl", "api", "models"],
        ),
        "pi" => (
            "https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/configuration.md",
            &["defaultProvider", "defaultModel", "enabledModels"],
        ),
        "qwen" if file.ends_with(".env") => (
            "https://qwenlm.github.io/qwen-code-docs/en/users/configuration/settings/",
            &["OPENAI_BASE_URL", "OPENAI_API_KEY"],
        ),
        "qwen" => (
            "https://qwenlm.github.io/qwen-code-docs/en/users/configuration/settings/",
            &["model", "modelProviders", "security", "envKey"],
        ),
        "aider" => (
            "https://aider.chat/docs/config/aider_conf.html",
            &["model", "openai-api-base", "openai-api-key"],
        ),
        "goose" => (
            "https://goose.vcorp.ai/docs/guides/environment-variables/",
            &["GOOSE_PROVIDER", "GOOSE_MODEL", "GOOSE_TEMPERATURE"],
        ),
        "zed" => (
            "https://zed.dev/docs/ai/use-api-access",
            &[
                "language_models",
                "openai_compatible",
                "api_url",
                "available_models",
            ],
        ),
        "vscode" => (
            "https://code.visualstudio.com/docs/agent-customization/language-models",
            &["vendor", "name", "models", "url"],
        ),
        "cursor" => (
            "https://prod.cursor.com/help/models-and-usage/api-keys",
            &[],
        ),
        "amp" => (
            "https://ampcode.com/docs/customize/mcp",
            &["amp.mcpServers"],
        ),
        "continue" => (
            "https://docs.continue.dev/reference",
            &["name", "version", "schema", "models", "provider", "apiBase"],
        ),
        "cline" => ("https://docs.cline.bot/mcp/mcp-overview", &["mcpServers"]),
        "roo" => (
            "https://roocodeinc.github.io/Roo-Code/features/mcp/using-mcp-in-roo/",
            &["mcpServers"],
        ),
        "kilo" => (
            "https://kilo.ai/docs/automate/mcp/using-in-kilo-code",
            &["mcpServers"],
        ),
        _ => ("", &[]),
    }
}

#[derive(Default)]
pub(super) struct Editor {
    agent: String,
    files: Vec<String>,
    index: usize,
    format: String,
    revision: String,
    initial: String,
    text: String,
    cursor: usize,
    anchor: Option<usize>,
    scroll: usize,
    focused: bool,
    status: String,
    failed: bool,
    closing: bool,
    undo: Vec<(String, usize)>,
}

impl Editor {
    fn dirty(&self) -> bool {
        self.initial != self.text
    }
    fn selection(&self) -> std::ops::Range<usize> {
        let anchor = self.anchor.unwrap_or(self.cursor);
        anchor.min(self.cursor)..anchor.max(self.cursor)
    }
    fn replace(&mut self, text: &str) {
        let range = self.selection();
        if self.text.len() - range.len() + text.len() > LIMIT {
            return;
        }
        if self.undo.len() == 16 {
            self.undo.remove(0);
        }
        self.undo.push((self.text.clone(), self.cursor));
        self.cursor = range.start + text.len();
        self.text.replace_range(range, text);
        self.anchor = None;
        self.status.clear();
    }
    fn line(&self) -> (usize, usize, usize) {
        let before = &self.text[..self.cursor];
        let start = before.rfind('\n').map_or(0, |offset| offset + 1);
        let end = self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |offset| self.cursor + offset);
        (before.bytes().filter(|&b| b == b'\n').count(), start, end)
    }
    fn complete(&mut self, key: &str, start: usize) {
        let (_, line_start, line_end) = self.line();
        let before = self.text[line_start..start].trim_start();
        let after = self.text[self.cursor..line_end].trim_start();
        let is_table = key.starts_with('[');
        let suffix_exists = after.starts_with([':', '=', '"', ']']);
        let mut end = self.cursor;
        if let Some(remainder) = self.text[self.cursor..line_end].chars().next()
            && !suffix_exists
            && (remainder.is_ascii_alphanumeric() || matches!(remainder, '_' | '-'))
        {
            end += self.text[self.cursor..line_end]
                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
                .unwrap_or(line_end - self.cursor);
        }
        let snippet = if before.is_empty() && !suffix_exists {
            match self.format.as_str() {
                "json" | "jsonc" => format!("\"{key}\": "),
                "toml" if !is_table => format!("{key} = "),
                "yaml" => format!("{key}: "),
                "env" => format!("{key}="),
                _ => key.to_owned(),
            }
        } else {
            key.to_owned()
        };
        self.cursor = end;
        self.anchor = Some(start);
        self.replace(&snippet);
    }
    fn key_event(
        &mut self,
        event: &Event,
        keys: &[&str],
        clipboard: &mut Option<String>,
    ) -> Option<&'static str> {
        if !self.focused {
            return None;
        }
        if let Event::Text(text) = event {
            self.replace(&text.replace("\r\n", "\n").replace('\r', "\n"));
            return None;
        }
        let Event::Key {
            code,
            control,
            shift,
            text,
        } = event
        else {
            return None;
        };
        if *control {
            match code {
                65 | 97 => {
                    self.anchor = Some(0);
                    self.cursor = self.text.len();
                }
                67 | 99 | 88 | 120 => {
                    let selection = self.selection();
                    if !selection.is_empty() {
                        *clipboard = Some(self.text[selection].to_owned());
                        if matches!(code, 88 | 120) {
                            self.replace("");
                        }
                    }
                }
                83 | 115 => return Some(SAVE),
                90 | 122 => {
                    if let Some((text, cursor)) = self.undo.pop() {
                        self.text = text;
                        self.cursor = cursor;
                        self.anchor = None;
                        self.status.clear();
                    }
                }
                _ => {}
            }
            return None;
        }
        let (_, start, end) = self.line();
        let left = self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        let right = self.cursor
            + self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        let next = match code {
            1001 => left,
            1002 => right,
            1005 => start,
            1006 => end,
            1003 | 1004 => {
                let column = self.text[start..self.cursor].chars().count();
                let (next_start, next_end) = if *code == 1003 && start > 0 {
                    let previous_end = start - 1;
                    (
                        self.text[..previous_end].rfind('\n').map_or(0, |i| i + 1),
                        previous_end,
                    )
                } else if *code == 1004 && end < self.text.len() {
                    (
                        end + 1,
                        self.text[end + 1..]
                            .find('\n')
                            .map_or(self.text.len(), |i| end + 1 + i),
                    )
                } else {
                    return None;
                };
                self.text[next_start..next_end]
                    .char_indices()
                    .nth(column)
                    .map_or(next_end, |(i, _)| next_start + i)
            }
            8 | 127 => {
                if self.selection().is_empty() {
                    self.anchor = Some(if *code == 8 { left } else { right });
                }
                self.replace("");
                return None;
            }
            9 => {
                let prefix = key_start(&self.text, self.cursor, start);
                let typed = &self.text[prefix..self.cursor];
                if let Some(key) = keys
                    .iter()
                    .find(|key| !typed.is_empty() && *key != &typed && key.starts_with(typed))
                {
                    self.complete(key, prefix);
                } else {
                    self.replace("  ");
                }
                return None;
            }
            13 => {
                let indent = self.text[start..end]
                    .chars()
                    .take_while(|c| matches!(c, ' ' | '\t'))
                    .collect::<String>();
                self.replace(&format!("\n{indent}"));
                return None;
            }
            _ => {
                if !text.is_empty() && text.chars().all(|ch| !ch.is_control()) {
                    self.replace(text);
                }
                return None;
            }
        };
        self.anchor = if *shift {
            Some(self.anchor.unwrap_or(self.cursor))
        } else {
            None
        };
        self.cursor = next;
        None
    }
}

fn key_start(text: &str, cursor: usize, start: usize) -> usize {
    text[start..cursor]
        .char_indices()
        .rev()
        .find(|&(_, ch)| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '[')))
        .map_or(start, |(offset, ch)| start + offset + ch.len_utf8())
}

impl App {
    pub(super) fn open_agent_config(&mut self, agent: &str) {
        self.ui = Ui::default();
        self.agent_config = Some(Editor {
            agent: agent.into(),
            ..Editor::default()
        });
        self.agent_config_request(LOAD);
    }
    pub(super) fn close_agent_config(&mut self) {
        if self.busy {
            return;
        }
        if let Some(editor) = &mut self.agent_config
            && editor.dirty()
        {
            editor.closing = true;
            self.repaint = true;
            return;
        }
        self.agent_config = None;
        self.ui = Ui::default();
        self.repaint = true;
        self.refresh();
    }
    fn discard_agent_config(&mut self) {
        if !self.busy {
            self.agent_config = None;
            self.ui = Ui::default();
            self.repaint = true;
            self.refresh();
        }
    }
    fn agent_config_request(&mut self, label: &'static str) {
        let Some(editor) = &self.agent_config else {
            return;
        };
        if self.busy || (label == SAVE && (!editor.dirty() || editor.revision.is_empty())) {
            return;
        }
        let mut args = words(&[
            "agents",
            "config",
            &editor.agent,
            "--index",
            &editor.index.to_string(),
        ]);
        let input = (label != LOAD).then(|| editor.text.as_bytes().to_vec());
        if label == CHECK {
            args.push("--check".into());
        }
        if label == SAVE {
            args.extend(words(&["--write", "--revision", &editor.revision]));
        }
        self.busy = true;
        self.repaint = true;
        let backend = self.backend.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = backend.run(&args, input);
            let _ = sender.send(Reply {
                label: label.into(),
                result,
                refresh: false,
                starts_gateway: false,
            });
        });
    }
    pub(super) fn accept_agent_config(&mut self, label: &str, result: Result<Json, String>) {
        let Some(editor) = &mut self.agent_config else {
            return;
        };
        editor.failed = result.is_err();
        match result {
            Err(error) => editor.status = error,
            Ok(data) if label == LOAD => {
                editor.files = data
                    .get("files")
                    .array()
                    .iter()
                    .map(|file| file.text().into())
                    .collect();
                editor.format = data.get("format").text().into();
                editor.revision = data.get("revision").text().into();
                editor.text = data.get("content").text().into();
                editor.initial = editor.text.clone();
                editor.cursor = 0;
                editor.anchor = None;
                editor.undo.clear();
                editor.scroll = 0;
                editor.focused = true;
                editor.status.clear();
            }
            Ok(data) if label == SAVE => {
                editor.revision = data.get("revision").text().into();
                editor.initial = editor.text.clone();
                editor.closing = false;
                editor.status = tr("agent_config.saved").into();
            }
            Ok(_) => editor.status = tr("agent_config.valid").into(),
        }
    }

    pub(super) fn agent_config_dialog(&mut self, c: &mut Canvas<'_>, event: Option<&Event>) {
        c.rect(Rect::new(0., 0., c.width, c.height), 0., 0x00000070);
        let w = (c.width - 40.).min(980.);
        let h = c.height - 38.;
        let x = (c.width - w) / 2.;
        let y = 19.;
        let inner = w - 42.;
        let l = x + 21.;
        c.rect(Rect::new(x, y, w, h), 13., SURFACE);
        let enabled = !self.busy;
        let Some(editor) = &mut self.agent_config else {
            return;
        };
        let loaded = !editor.revision.is_empty();
        c.ellipsis(
            agent_identity::display_name(&editor.agent),
            l,
            y + 17.,
            inner - 42.,
            24,
            true,
            TEXT,
        );
        let file = editor.files.get(editor.index).cloned().unwrap_or_default();
        let (url, keys) = guide(&editor.agent, &file);
        c.ellipsis(&file, l, y + 57., inner - 88., 13, false, MUTED);
        for (idx, (glyph, delta)) in [("‹", -1isize), ("›", 1)].into_iter().enumerate() {
            let next = editor
                .index
                .checked_add_signed(delta)
                .filter(|n| *n < editor.files.len());
            if self.ui.button(
                c,
                26100 + idx as u64,
                Rect::new(l + inner - 78. + 39. * idx as f32, y + 49., 36., 27.),
                glyph,
                Style::Quiet,
                enabled && !editor.dirty() && next.is_some(),
            ) {
                editor.index = next.unwrap_or_default();
                editor.revision.clear();
                self.agent_config_request(LOAD);
                return;
            }
        }
        if self.ui.button(
            c,
            26102,
            Rect::new(l + inner - 36., y + 10., 36., 33.),
            "×",
            Style::Quiet,
            enabled,
        ) {
            self.close_agent_config();
            return;
        }
        if self.ui.button(
            c,
            26103,
            Rect::new(l, y + 83., 147., 33.),
            tr("agent_config.docs"),
            Style::Secondary,
            enabled && !url.is_empty(),
        ) && let Err(error) = provider_oauth::open_browser(url)
        {
            editor.status = error;
            editor.failed = true;
        }
        if self.ui.button(
            c,
            26104,
            Rect::new(l + 156., y + 83., 188., 33.),
            tr("agent_config.copy"),
            Style::Secondary,
            enabled && loaded,
        ) {
            let paths = editor.files.join("\n");
            self.ui.clipboard = Some(i18n::message(
                "agent_config.prompt",
                &[
                    ("agent", &editor.agent),
                    ("paths", &paths),
                    ("url", url),
                    ("nl", "\n"),
                ],
            ));
            editor.status = tr("agent_config.copied").into();
            editor.failed = false;
        }

        let hint_key = match editor.agent.as_str() {
            "codex" => "agent_config.hint_codex",
            "claude" => "agent_config.hint_claude",
            "gemini" => "agent_config.hint_gemini",
            "opencode" | "pi" | "qwen" | "aider" | "goose" | "zed" | "vscode" | "continue" => {
                "agent_config.hint_provider"
            }
            "cline" | "roo" | "kilo" | "amp" => "agent_config.hint_mcp",
            _ => "agent_config.hint_default",
        };
        c.ellipsis(tr(hint_key), l, y + 123., inner, 12, false, MUTED);
        let area = Rect::new(l, y + 146., inner, (h - 268.).max(32.));
        let rows = (area.h / LINE_HEIGHT).max(1.) as usize;
        let mut submit = None;
        if let Some(event) = event.filter(|_| enabled && loaded) {
            match event {
                Event::Down(mx, my) => {
                    editor.focused = area.contains(*mx, *my);
                    if editor.focused {
                        let row = editor.scroll + ((*my - area.y) / LINE_HEIGHT) as usize;
                        let mut offset = 0;
                        for (n, line) in editor.text.split('\n').enumerate() {
                            if n == row {
                                let (_, start, _) = editor.line();
                                let current =
                                    c.measure(&editor.text[start..editor.cursor], FONT, false);
                                let horizontal = (current - (area.w - 75.)).max(0.);
                                let xpos = *mx - area.x - 55. + horizontal;
                                editor.cursor = offset + char_at_x(c, line, xpos);
                                editor.anchor = None;
                                break;
                            }
                            offset += line.len() + 1;
                        }
                    }
                }
                Event::Scroll(delta) => {
                    editor.scroll = editor
                        .scroll
                        .saturating_add_signed(delta.signum() as isize * 3)
                        .min(editor.text.lines().count().saturating_sub(rows));
                }
                Event::Key { .. } | Event::Text(_) => {
                    submit = editor.key_event(event, keys, &mut self.ui.clipboard);
                    if editor.focused {
                        let row = editor.line().0;
                        editor.scroll = editor.scroll.min(row).max(row.saturating_sub(rows - 1));
                    }
                }
                _ => {}
            }
        }
        c.rect(area, 7., BG);
        c.clip(area);
        let selection = editor.selection();
        let cursor_row = editor.line().0;
        let offset_x = c.measure(&editor.text[editor.line().1..editor.cursor], FONT, false);
        let horizontal = (offset_x - (area.w - 75.)).max(0.);
        let mut base = 0;
        for (row, line) in editor.text.split('\n').enumerate() {
            if row >= editor.scroll + rows {
                break;
            }
            if row >= editor.scroll {
                let top = area.y + (row - editor.scroll) as f32 * LINE_HEIGHT + 2.;
                let text_x = area.x + 55. - horizontal;
                if selection.start < base + line.len() && selection.end > base {
                    let a = selection.start.saturating_sub(base);
                    let b = (selection.end - base).min(line.len());
                    let left = c.measure(&line[..a], FONT, false);
                    c.rect(
                        Rect::new(
                            text_x + left,
                            top,
                            c.measure(&line[a..b], FONT, false),
                            LINE_HEIGHT,
                        ),
                        2.,
                        ACCENT_SOFT,
                    );
                }
                let first = if horizontal > 0. {
                    char_at_x(c, line, (horizontal - 12.).max(0.))
                } else {
                    0
                };
                let visible = &line[first..];
                let end = visible
                    .char_indices()
                    .nth(480)
                    .map_or(visible.len(), |(i, _)| i);
                paint_line(
                    c,
                    &visible[..end],
                    text_x + c.measure(&line[..first], FONT, false),
                    top,
                );
                c.rect(Rect::new(area.x, top, 47., LINE_HEIGHT), 0., BG);
                c.text(
                    &(row + 1).to_string(),
                    area.x + 9.,
                    top + 2.,
                    12,
                    false,
                    MUTED,
                );
                if row == cursor_row && editor.focused {
                    c.rect(Rect::new(text_x + offset_x, top + 2., 1.5, 17.), 0., ACCENT);
                }
            }
            base += line.len() + 1;
        }
        c.unclip();

        let suggestions_y = area.y + area.h + 3.;
        let mut next_x = l;
        if loaded {
            let start = key_start(&editor.text, editor.cursor, editor.line().1);
            let typed = &editor.text[start..editor.cursor];
            let options: Vec<_> = keys
                .iter()
                .copied()
                .filter(|key| typed.is_empty() || key.starts_with(typed))
                .collect();
            for (i, key) in options.into_iter().take(5).enumerate() {
                let width = c.measure(key, 12, false) + 19.;
                if next_x + width > l + inner {
                    break;
                }
                if self.ui.button(
                    c,
                    26120 + i as u64,
                    Rect::new(next_x, suggestions_y, width, 26.),
                    key,
                    Style::Quiet,
                    enabled,
                ) {
                    editor.complete(key, start);
                    editor.focused = true;
                }
                next_x += width + 4.;
            }
        }
        let status = if editor.closing {
            tr("agent_config.discard_hint")
        } else if !enabled {
            tr("status.loading")
        } else if editor.status.is_empty() {
            tr("agent_config.help")
        } else {
            &editor.status
        };
        c.ellipsis(
            status,
            l,
            y + h - 77.,
            inner,
            13,
            false,
            if editor.failed { RED } else { MUTED },
        );
        let footer = y + h - 49.;
        if editor.closing
            && self.ui.button(
                c,
                26105,
                Rect::new(l, footer, 155., 35.),
                tr("agent_config.discard"),
                Style::Danger,
                enabled,
            )
        {
            self.discard_agent_config();
            return;
        }
        if self.ui.button(
            c,
            26106,
            Rect::new(l + inner - 205., footer, 99., 35.),
            tr("agent_config.check"),
            Style::Secondary,
            enabled && loaded,
        ) {
            submit = Some(CHECK);
        }
        if self.ui.button(
            c,
            26107,
            Rect::new(l + inner - 98., footer, 98., 35.),
            tr("agent_config.save"),
            Style::Primary,
            enabled && loaded && editor.dirty(),
        ) {
            submit = Some(SAVE);
        }
        if let Some(label) = submit {
            self.agent_config_request(label);
        }
    }
}

fn char_at_x(c: &Canvas<'_>, line: &str, x: f32) -> usize {
    if x <= 0. {
        return 0;
    }
    let offsets: Vec<_> = line.char_indices().map(|(i, _)| i).collect();
    let index = offsets.partition_point(|&i| c.measure(&line[..i], FONT, false) < x);
    offsets.get(index).copied().unwrap_or(line.len())
}

fn paint_line(c: &mut Canvas<'_>, line: &str, mut x: f32, y: f32) {
    let mut rest = line;
    while !rest.is_empty() {
        let first = rest.chars().next().unwrap_or_default();
        let (length, color) = if first == '#' || rest.starts_with("//") {
            (rest.len(), MUTED)
        } else if first == '"' || first == '\'' {
            let mut escaped = false;
            let end = rest
                .char_indices()
                .skip(1)
                .find(|&(_, ch)| {
                    if !escaped && ch == first {
                        return true;
                    }
                    escaped = !escaped && ch == '\\';
                    false
                })
                .map_or(rest.len(), |(position, _)| position + 1);
            let key = rest[end..].trim_start().starts_with([':', '=']);
            (end, if key { ACCENT } else { TEXT })
        } else if first.is_ascii_digit() || matches!(first, '[' | ']' | '{' | '}' | ':' | '=') {
            (first.len_utf8(), ACCENT)
        } else {
            (first.len_utf8(), TEXT)
        };
        let (token, next) = rest.split_at(length);
        x += c.text(token, x, y, FONT, false, color);
        rest = next;
    }
}
