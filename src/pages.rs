use crate::*;
mod config;
impl App {
    pub(super) fn overview(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let status = self.state.get("status").clone();
        let error = status.get("error").text();
        let healthy = status.get("healthy").boolean()
            || status.get("health").get("status").text() == "ok"
            || status.get("status").text() == "ok";
        let available = error.is_empty() && !matches!(status, Json::Null);
        card(c, Rect::new(x, 130., width, 186.));
        c.rect(
            Rect::new(x + 24., 154., 9., 9.),
            4.5,
            if healthy { GREEN } else { MUTED },
        );
        c.text(
            if healthy {
                "GATEWAY RUNNING"
            } else if available {
                "GATEWAY READY"
            } else {
                "GATEWAY UNAVAILABLE"
            },
            x + 43.,
            147.,
            12,
            true,
            if healthy { GREEN } else { MUTED },
        );
        c.text(
            value(&status, "listen", "127.0.0.1:9415"),
            x + 24.,
            180.,
            28,
            true,
            TEXT,
        );
        c.text(
            &format!(
                "zcalld {}  ·  {}",
                value(&status, "version", "not detected"),
                if healthy {
                    "accepting requests"
                } else {
                    "service status is shown by the CLI"
                }
            ),
            x + 24.,
            224.,
            14,
            false,
            MUTED,
        );
        let enabled = self.enabled();
        for (index, (label, command)) in
            [("Start", "start"), ("Restart", "restart"), ("Stop", "stop")]
                .iter()
                .enumerate()
        {
            if self.ui.button(
                c,
                100 + index as u64,
                Rect::new(x + 24. + index as f32 * 116., 264., 104., 36.),
                label,
                if index == 0 {
                    Style::Primary
                } else {
                    Style::Secondary
                },
                enabled && available,
            ) {
                self.action(
                    &format!("{label} gateway"),
                    words(&["service", command]),
                    None,
                );
            }
        }
        let gap = 16.;
        let cw = (width - gap * 2.) / 3.;
        for (index, (label, number)) in [
            ("Providers", count(&self.state, "providers")),
            ("AI tools", count(&self.state, "agents")),
            ("Plugins", count(&self.state, "plugins")),
        ]
        .iter()
        .enumerate()
        {
            let left = x + index as f32 * (cw + gap);
            card(c, Rect::new(left, 336., cw, 112.));
            c.text(&number.to_string(), left + 22., 353., 30, true, TEXT);
            c.text(label, left + 22., 405., 14, false, MUTED);
        }
        c.text(
            "Runs locally. Opens only when you need it.",
            x,
            479.,
            19,
            true,
            TEXT,
        );
        c.text(
            "Closing this window leaves the gateway service running.",
            x,
            513.,
            15,
            false,
            MUTED,
        );
        c.ellipsis(
            &format!("GPU: {}", self.gpu),
            x,
            547.,
            width,
            13,
            false,
            MUTED,
        );
        if !available {
            c.ellipsis(
                "Install zcalld, then Refresh. Use --zcalld PATH for a custom executable.",
                x,
                580.,
                width,
                14,
                false,
                RED,
            );
        }
    }
    pub(super) fn agents(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let enabled = self.enabled();
        let items = self.state.get("agents").get("agents").array().to_vec();
        field(
            &mut self.ui,
            c,
            500,
            x,
            126.,
            (width - 156.).max(240.),
            "Gateway route",
            &mut self.route,
            "local",
            false,
            enabled,
        );
        if self.ui.button(
            c,
            501,
            Rect::new(x + width - 132., 149., 132., 42.),
            "Detect tools",
            Style::Secondary,
            enabled,
        ) {
            self.action("Detect AI tools", words(&["agents", "detect"]), None);
        }
        let rows = ((c.height - 290.) / 92.).max(1.) as usize;
        self.scroll = self.scroll.min(items.len().saturating_sub(rows));
        if items.is_empty() {
            empty(
                c,
                x,
                240.,
                width,
                "No AI tools detected",
                "Use Detect tools after installing a supported client.",
            );
        }
        for (index, item) in items.iter().enumerate().skip(self.scroll).take(rows) {
            let y = 222. + (index - self.scroll) as f32 * 92.;
            let name = value(item, "name", "tool");
            card(c, Rect::new(x, y, width, 80.));
            c.text(name, x + 18., y + 12., 18, true, TEXT);
            c.ellipsis(
                value(item, "path", "configuration path not detected"),
                x + 18.,
                y + 44.,
                width - 260.,
                13,
                false,
                MUTED,
            );
            c.text(
                if item.get("configured").boolean() {
                    "gateway configured"
                } else if item.get("installed").boolean() {
                    "installed"
                } else {
                    "not installed"
                },
                x + width - 396.,
                y + 16.,
                12,
                false,
                MUTED,
            );
            if self.ui.button(
                c,
                20000 + index as u64 * 2,
                Rect::new(x + width - 226., y + 22., 100., 36.),
                "Apply route",
                Style::Primary,
                enabled,
            ) {
                let route = if self.route.is_empty() {
                    "local"
                } else {
                    &self.route
                };
                self.ask(
                    &format!("Configure {name}?"),
                    words(&["agents", "apply", name, "--route", route]),
                );
            }
            if self.ui.button(
                c,
                20001 + index as u64 * 2,
                Rect::new(x + width - 116., y + 22., 96., 36.),
                "Revert",
                Style::Secondary,
                enabled && item.get("configured").boolean(),
            ) {
                self.ask(
                    &format!("Revert {name}?"),
                    words(&["agents", "revert", name]),
                );
            }
        }
    }
    pub(super) fn privacy(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let state = self.state.get("privacy").clone();
        let mode = value(
            &state,
            "mode",
            if state.get("enabled").boolean() {
                "secrets"
            } else {
                "off"
            },
        );
        let enabled = self.enabled();
        for (index, (id, title, description)) in [
            (
                "off",
                "Off",
                "Forward request text without automatic sanitization.",
            ),
            (
                "secrets",
                "Secrets",
                "Sanitize detected credentials, tokens and secret values.",
            ),
            (
                "personal",
                "Secrets + personal",
                "Also sanitize supported personal information patterns.",
            ),
        ]
        .iter()
        .enumerate()
        {
            let y = 132. + index as f32 * 117.;
            card(c, Rect::new(x, y, width, 99.));
            if mode == *id {
                c.rect(Rect::new(x, y, 4., 99.), 2., BLUE);
            }
            c.text(title, x + 24., y + 18., 20, true, TEXT);
            c.text(description, x + 24., y + 55., 14, false, MUTED);
            if self.ui.button(
                c,
                800 + index as u64,
                Rect::new(x + width - 130., y + 27., 106., 38.),
                if mode == *id { "Selected" } else { "Use mode" },
                if mode == *id {
                    Style::Quiet
                } else {
                    Style::Secondary
                },
                enabled && mode != *id,
            ) {
                self.action("Set privacy mode", words(&["privacy", "set", id]), None);
            }
        }
        c.text("Sanitization has limits.", x, 505., 18, true, TEXT);
        c.text(
            "Review provider settings and avoid sending information you cannot share.",
            x,
            539.,
            14,
            false,
            MUTED,
        );
        c.text(
            &format!(
                "Model-assisted detection: {}",
                if state.get("model_auto").boolean() {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
            x,
            575.,
            14,
            false,
            MUTED,
        );
    }
    pub(super) fn updates(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let enabled = self.enabled();
        let versions = [
            (
                "Gateway".to_owned(),
                value(self.state.get("status"), "version", "not detected").to_owned(),
                value(
                    &self.gateway_update,
                    "latest",
                    value(&self.gateway_update, "latest_version", "not checked"),
                )
                .to_owned(),
            ),
            (
                "Control panel".to_owned(),
                VERSION.to_owned(),
                value(&self.gui_update, "latest", "not checked").to_owned(),
            ),
        ];
        for (index, (title, current, latest)) in versions.iter().enumerate() {
            let y = 132. + index as f32 * 197.;
            card(c, Rect::new(x, y, width, 177.));
            c.text(title, x + 24., y + 18., 22, true, TEXT);
            c.text(
                &format!("Installed {current}  ·  Latest {latest}"),
                x + 24.,
                y + 58.,
                15,
                false,
                MUTED,
            );
            if self.ui.button(
                c,
                850 + index as u64 * 2,
                Rect::new(x + 24., y + 109., 142., 40.),
                "Check update",
                Style::Secondary,
                enabled,
            ) {
                if index == 0 {
                    self.action("Check gateway update", words(&["update", "check"]), None);
                } else {
                    self.self_update(false);
                }
            }
            if self.ui.button(
                c,
                851 + index as u64 * 2,
                Rect::new(x + 180., y + 109., 142., 40.),
                "Update now",
                Style::Primary,
                enabled,
            ) {
                if index == 0 {
                    self.ask("Update gateway?", words(&["update", "apply"]));
                } else {
                    self.self_update(true);
                }
            }
        }
        c.text(
            "System-managed installs follow your package manager.",
            x,
            556.,
            16,
            true,
            TEXT,
        );
        c.text(
            "AUR: yay -Syu zcalld-bin zcall-gui-bin",
            x,
            590.,
            14,
            false,
            MUTED,
        );
    }
}
