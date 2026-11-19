use crate::*;
impl App {
    pub(crate) fn providers(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let form_w = (width * 0.45).max(280.);
        let list_w = width - form_w - 24.;
        let fx = x + list_w + 24.;
        let enabled = self.enabled();
        let items = self
            .state
            .get("providers")
            .get("providers")
            .array()
            .to_vec();
        c.text(
            &format!("{} configured", items.len()),
            x,
            127.,
            14,
            true,
            MUTED,
        );
        let rows = ((c.height - 225.) / 96.).max(1.) as usize;
        self.scroll = self.scroll.min(items.len().saturating_sub(rows));
        if items.is_empty() {
            empty(
                c,
                x,
                176.,
                list_w,
                "No providers yet",
                "Add an endpoint using the form.",
            );
        }
        for (index, item) in items.iter().enumerate().skip(self.scroll).take(rows) {
            let y = 160. + (index - self.scroll) as f32 * 96.;
            let name = value(item, "name", "provider");
            card(c, Rect::new(x, y, list_w, 84.));
            c.ellipsis(name, x + 16., y + 11., list_w - 130., 16, true, TEXT);
            c.ellipsis(
                value(item, "base_url", ""),
                x + 16.,
                y + 38.,
                list_w - 28.,
                12,
                false,
                MUTED,
            );
            c.text(
                if item.get("configured").boolean() {
                    "credential ready"
                } else {
                    "no credential"
                },
                x + 16.,
                y + 61.,
                11,
                false,
                MUTED,
            );
            if self.ui.button(
                c,
                10000 + index as u64 * 3,
                Rect::new(x + list_w - 110., y + 8., 44., 28.),
                "Test",
                Style::Quiet,
                enabled,
            ) {
                self.action("Test provider", words(&["providers", "test", name]), None);
            }
            if self.ui.button(
                c,
                10001 + index as u64 * 3,
                Rect::new(x + list_w - 62., y + 8., 48., 28.),
                "Edit",
                Style::Quiet,
                enabled,
            ) {
                self.provider.name = name.into();
                self.provider.url = item.get("base_url").text().into();
                self.provider.environment = item.get("api_key_env").text().into();
                self.provider.protocol = protocols()
                    .iter()
                    .position(|s| *s == item.get("protocol").text())
                    .unwrap_or(0);
            }
            if self.ui.button(
                c,
                10002 + index as u64 * 3,
                Rect::new(x + list_w - 76., y + 56., 62., 22.),
                "Remove",
                Style::Quiet,
                enabled,
            ) {
                self.ask(
                    &format!("Remove {name}?"),
                    words(&["providers", "remove", name]),
                );
            }
        }
        card(c, Rect::new(fx, 124., form_w, (c.height - 196.).min(570.)));
        c.text("Add / update provider", fx + 20., 142., 18, true, TEXT);
        field(
            &mut self.ui,
            c,
            400,
            fx + 20.,
            179.,
            form_w - 40.,
            "Name",
            &mut self.provider.name,
            "my-provider",
            false,
            enabled,
        );
        field(
            &mut self.ui,
            c,
            401,
            fx + 20.,
            250.,
            form_w - 40.,
            "Base URL",
            &mut self.provider.url,
            "https://api.example.com/v1",
            false,
            enabled,
        );
        c.text("Protocol", fx + 20., 321., 12, true, MUTED);
        if self.ui.button(
            c,
            402,
            Rect::new(fx + 20., 344., form_w - 40., 36.),
            protocols()[self.provider.protocol],
            Style::Secondary,
            enabled,
        ) {
            self.provider.protocol = (self.provider.protocol + 1) % protocols().len();
        }
        field(
            &mut self.ui,
            c,
            403,
            fx + 20.,
            394.,
            form_w - 40.,
            "Default model (optional)",
            &mut self.provider.model,
            "model name",
            false,
            enabled,
        );
        field(
            &mut self.ui,
            c,
            404,
            fx + 20.,
            465.,
            form_w - 40.,
            "API key (optional, saved locally)",
            &mut self.provider.key,
            "leave blank to keep current",
            true,
            enabled,
        );
        if self.ui.button(
            c,
            405,
            Rect::new(fx + 20., 548., form_w - 40., 40.),
            "Save provider",
            Style::Primary,
            enabled
                && !self.provider.name.trim().is_empty()
                && !self.provider.url.trim().is_empty(),
        ) {
            let mut args = words(&[
                "providers",
                "add",
                &self.provider.name,
                "--base-url",
                &self.provider.url,
                "--protocol",
                protocols()[self.provider.protocol],
            ]);
            if !self.provider.model.is_empty() {
                args.extend(words(&["--model", &self.provider.model]));
            }
            if !self.provider.environment.is_empty() {
                args.extend(words(&["--api-key-env", &self.provider.environment]));
            }
            let secret = if self.provider.key.is_empty() {
                None
            } else {
                args.push("--api-key-stdin".into());
                Some(std::mem::take(&mut self.provider.key).into_bytes())
            };
            self.action("Save provider", args, secret);
        }
    }
    pub(crate) fn plugins(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let enabled = self.enabled();
        let items = self.state.get("plugins").get("plugins").array().to_vec();
        let form_w = (width * 0.42).max(280.);
        let list_w = width - form_w - 24.;
        let fx = x + list_w + 24.;
        c.text("Installed plugins", x, 127., 15, true, MUTED);
        let rows = ((c.height - 230.) / 94.).max(1.) as usize;
        self.scroll = self.scroll.min(items.len().saturating_sub(rows));
        if items.is_empty() {
            empty(
                c,
                x,
                176.,
                list_w,
                "Core stays small",
                "Install a plugin, then enable it.",
            );
        }
        for (index, item) in items.iter().enumerate().skip(self.scroll).take(rows) {
            let y = 158. + (index - self.scroll) as f32 * 94.;
            let name = value(item, "name", "plugin");
            let on = item.get("enabled").boolean();
            card(c, Rect::new(x, y, list_w, 82.));
            c.ellipsis(name, x + 16., y + 10., list_w - 124., 16, true, TEXT);
            c.ellipsis(
                &format!(
                    "{} · {}",
                    value(item, "kind", "native"),
                    value(item, "version", "unversioned")
                ),
                x + 16.,
                y + 38.,
                list_w - 100.,
                12,
                false,
                MUTED,
            );
            if self.ui.button(
                c,
                30000 + index as u64 * 3,
                Rect::new(x + list_w - 100., y + 10., 84., 30.),
                if on { "Disable" } else { "Enable" },
                if on { Style::Secondary } else { Style::Primary },
                enabled,
            ) {
                let action = if on { "disable" } else { "enable" };
                self.ask(
                    &format!("{action} {name}?"),
                    words(&["plugins", action, name]),
                );
            }
            if self.ui.button(
                c,
                30001 + index as u64 * 3,
                Rect::new(x + 16., y + 59., 54., 20.),
                "Select",
                Style::Quiet,
                enabled,
            ) {
                self.plugin.name = name.into();
            }
            if self.ui.button(
                c,
                30002 + index as u64 * 3,
                Rect::new(x + list_w - 76., y + 55., 60., 23.),
                "Remove",
                Style::Quiet,
                enabled,
            ) {
                self.ask(
                    &format!("Remove {name}?"),
                    words(&["plugins", "remove", name]),
                );
            }
        }
        card(c, Rect::new(fx, 124., form_w, (c.height - 196.).min(580.)));
        c.text("Install / update", fx + 20., 142., 18, true, TEXT);
        field(
            &mut self.ui,
            c,
            750,
            fx + 20.,
            179.,
            form_w - 40.,
            "Local path, Git URL or npm package",
            &mut self.plugin.source,
            "/path/to/plugin.so",
            false,
            enabled,
        );
        field(
            &mut self.ui,
            c,
            751,
            fx + 20.,
            250.,
            form_w - 40.,
            "Pinned Git commit / npm version",
            &mut self.plugin.revision,
            "required for remote sources",
            false,
            enabled,
        );
        field(
            &mut self.ui,
            c,
            752,
            fx + 20.,
            321.,
            form_w - 40.,
            "Name (optional for install)",
            &mut self.plugin.name,
            "select a plugin to update",
            false,
            enabled,
        );
        field(
            &mut self.ui,
            c,
            753,
            fx + 20.,
            392.,
            form_w - 40.,
            "SHA-256 (optional)",
            &mut self.plugin.sha256,
            "verify a downloaded library",
            false,
            enabled,
        );
        if self.ui.button(
            c,
            754,
            Rect::new(fx + 20., 482., form_w - 40., 40.),
            "Install plugin",
            Style::Primary,
            enabled && !self.plugin.source.is_empty(),
        ) {
            let mut args = words(&["plugins", "install", &self.plugin.source]);
            if !self.plugin.revision.is_empty() {
                args.extend(words(&["--revision", &self.plugin.revision]));
            }
            if !self.plugin.name.is_empty() {
                args.extend(words(&["--name", &self.plugin.name]));
            }
            if !self.plugin.sha256.is_empty() {
                args.extend(words(&["--sha256", &self.plugin.sha256]));
            }
            self.ask("Install plugin?", args);
        }
        if self.ui.button(
            c,
            755,
            Rect::new(fx + 20., 536., form_w - 40., 36.),
            "Update selected",
            Style::Secondary,
            enabled && !self.plugin.name.is_empty() && !self.plugin.revision.is_empty(),
        ) {
            let npm = items.iter().any(|item| {
                item.get("name").text() == self.plugin.name && item.get("kind").text() == "npm"
            });
            self.ask(
                "Update selected plugin?",
                words(&[
                    "plugins",
                    "update",
                    &self.plugin.name,
                    if npm { "--version" } else { "--revision" },
                    &self.plugin.revision,
                ]),
            );
        }
    }
}
