use crate::*;
impl App {
    pub(super) fn overview(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let state = self.state.get("status").clone();
        let healthy = state.get("healthy").boolean();
        let available = state.get("error").text().is_empty() && !matches!(state, Json::Null);
        let center = (x + width / 2.).round();
        let top = ((c.height - 600.) * 0.28 + 68.).round().clamp(68., 220.);
        let halo = if c.theme == zui::Theme::Dark {
            0x1d2a40ff
        } else {
            0xf3f7ffff
        };
        let edge = if c.theme == zui::Theme::Dark {
            0x283a55ff
        } else {
            0xeaf0fcff
        };
        c.rect(Rect::new(center - 108., top, 216., 216.), 108., edge);
        c.rect(Rect::new(center - 107., top + 1., 214., 214.), 107., halo);
        brand(c, Rect::new(center - 96., top + 12., 192., 192.), halo);
        let status = if healthy {
            tr("status.connected")
        } else {
            tr("status.disconnected")
        };
        let status_width = c.measure(status, 13, false);
        c.rect(
            Rect::new(
                (center - status_width / 2. - 14.).round(),
                top + 256.,
                6.,
                6.,
            ),
            3.,
            if healthy { GREEN } else { MUTED },
        );
        c.text(
            status,
            (center - status_width / 2.).round(),
            top + 249.,
            13,
            false,
            if healthy { GREEN } else { MUTED },
        );
        let heading = if healthy {
            tr("overview.running")
        } else {
            tr("overview.stopped")
        };
        let heading_width = c.measure(heading, 28, true);
        c.text(
            heading,
            (center - heading_width / 2.).round(),
            top + 286.,
            28,
            true,
            TEXT,
        );
        let address = value(&state, "listen", "127.0.0.1:9415");
        let address_width = c.measure(address, 16, false);
        c.text(
            address,
            (center - address_width / 2.).round(),
            top + 333.,
            16,
            false,
            MUTED,
        );
        let detail = if healthy {
            tr("overview.ready_help")
        } else if available {
            tr("overview.start_help")
        } else {
            tr("overview.install_help")
        };
        let detail_width = c.measure(detail, 14, false);
        c.ellipsis(
            detail,
            (center - detail_width.min(width) / 2.).round(),
            top + 371.,
            width,
            14,
            false,
            MUTED,
        );
        let label = if healthy {
            tr("action.manage_providers")
        } else if available {
            tr("action.start_gateway")
        } else {
            tr("action.recheck")
        };
        if self.ui.button(
            c,
            100,
            Rect::new(center - 78., top + 421., 156., 44.),
            label,
            Style::Primary,
            self.enabled(),
        ) {
            if healthy {
                self.page = 1;
            } else if available {
                self.ask(
                    tr("action.start_service_confirm"),
                    words(&["service", "start"]),
                );
            } else {
                self.refresh();
            }
        }
        if available
            && self.ui.button(
                c,
                101,
                Rect::new(center - 78., top + 479., 156., 36.),
                tr("action.service_settings"),
                Style::Quiet,
                self.enabled(),
            )
        {
            self.service_panel = true;
        }
    }
    pub(super) fn providers(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let enabled = self.enabled();
        if self.ui.button(
            c,
            400,
            Rect::new(x + width - 142., 35., 142., 42.),
            tr("action.add_provider"),
            Style::Primary,
            enabled,
        ) {
            self.clear_key();
            self.provider = ProviderForm::default();
            self.provider_edit = false;
            self.provider_panel = 2;
        }
        let items = self
            .state
            .get("providers")
            .get("providers")
            .array()
            .to_vec();
        if items.is_empty() {
            empty(
                c,
                Rect::new(x, 140., width, 172.),
                tr("providers.empty"),
                tr("providers.empty_help"),
            );
            return;
        }
        c.text(
            &i18n::message("providers.count", &[("count", &items.len().to_string())]),
            x,
            125.,
            13,
            false,
            MUTED,
        );
        let rows = ((c.height - 235.) / 80.).max(1.) as usize;
        self.scroll = self.scroll.min(items.len().saturating_sub(rows));
        let visible = items.len().saturating_sub(self.scroll).min(rows);
        card(c, Rect::new(x, 157., width, visible as f32 * 80.));
        for (index, item) in items.iter().enumerate().skip(self.scroll).take(rows) {
            let y = 157. + (index - self.scroll) as f32 * 80.;
            let r = Rect::new(x + 1., y + 1., width - 2., 78.);
            if self
                .ui
                .button(c, 10000 + index as u64, r, "", Style::Quiet, enabled)
            {
                self.clear_key();
                self.provider = ProviderForm {
                    name: value(item, "name", "provider").into(),
                    url: item.get("base_url").text().into(),
                    model: item.get("model").text().into(),
                    environment: item.get("api_key_env").text().into(),
                    key: String::new(),
                    protocol: protocols()
                        .iter()
                        .position(|p| *p == item.get("protocol").text())
                        .unwrap_or(0),
                };
                self.provider_edit = true;
                self.provider_panel = 1;
            }
            c.ellipsis(
                value(item, "name", tr("providers.unnamed")),
                x + 22.,
                y + 16.,
                width - 220.,
                17,
                true,
                TEXT,
            );
            c.ellipsis(
                value(item, "base_url", tr("providers.no_address")),
                x + 22.,
                y + 46.,
                width - 220.,
                13,
                false,
                MUTED,
            );
            let protocol = match item.get("protocol").text() {
                "chat" => "Chat",
                "responses" => "Responses",
                "anthropic" => "Claude",
                "gemini" => "Gemini",
                "completions" => "Completions",
                "interactions" => "Interactions",
                other => other,
            };
            let pill = (c.measure(protocol, 12, false) + 20.).round();
            let px = x + width - pill - 53.;
            c.rect(Rect::new(px, y + 28., pill, 25.), 6., BG);
            c.text(protocol, px + 10., y + 33., 12, false, MUTED);
            c.line(
                (x + width - 29., y + 35.),
                (x + width - 25., y + 39.),
                1.5,
                MUTED,
            );
            c.line(
                (x + width - 25., y + 39.),
                (x + width - 29., y + 43.),
                1.5,
                MUTED,
            );
            if index - self.scroll + 1 < visible {
                c.rect(Rect::new(x + 22., y + 79., width - 44., 1.), 0., LINE);
            }
        }
    }

    pub(super) fn agents(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let enabled = self.enabled();
        if self.ui.button(
            c,
            501,
            Rect::new(x + width - 130., 35., 130., 42.),
            tr("action.detect_agents"),
            Style::Secondary,
            enabled,
        ) {
            self.action("Detect AI tools", words(&["agents", "detect"]), None);
        }
        c.text(tr("agents.route"), x, 129., 13, true, TEXT);
        self.ui.input(
            c,
            500,
            Rect::new(x + 86., 117., 270., 42.),
            &mut self.route,
            "local",
            false,
            enabled,
        );
        let items = self.state.get("agents").get("agents").array().to_vec();
        let rows = ((c.height - 274.) / 90.).max(1.) as usize;
        self.scroll = self.scroll.min(items.len().saturating_sub(rows));
        if items.is_empty() {
            empty(
                c,
                Rect::new(x, 189., width, 170.),
                tr("agents.empty"),
                tr("agents.empty_help"),
            );
        }
        for (index, item) in items.iter().enumerate().skip(self.scroll).take(rows) {
            let y = 189. + (index - self.scroll) as f32 * 90.;
            let name = value(item, "name", "tool");
            let configured = item.get("configured").boolean();
            let installed = item.get("installed").boolean();
            card(c, Rect::new(x, y, width, 77.));
            c.text(
                match name {
                    "claude" => "Claude Code",
                    "codex" => "Codex",
                    _ => name,
                },
                x + 22.,
                y + 13.,
                18,
                true,
                TEXT,
            );
            c.ellipsis(
                if configured {
                    tr("agents.connected")
                } else if installed {
                    tr("agents.available")
                } else {
                    tr("agents.not_installed")
                },
                x + 22.,
                y + 45.,
                width - 210.,
                13,
                false,
                MUTED,
            );
            if self.ui.button(
                c,
                20000 + index as u64 * 2,
                Rect::new(x + width - 122., y + 19., 100., 38.),
                if configured {
                    tr("action.change_route")
                } else {
                    tr("action.connect")
                },
                if configured {
                    Style::Secondary
                } else {
                    Style::Primary
                },
                enabled && installed,
            ) {
                let route = if self.route.is_empty() {
                    "local"
                } else {
                    &self.route
                };
                self.ask(
                    &i18n::message("action.connect_named", &[("name", name)]),
                    words(&["agents", "apply", name, "--route", route]),
                );
            }
            if configured
                && self.ui.button(
                    c,
                    20001 + index as u64 * 2,
                    Rect::new(x + width - 216., y + 19., 80., 38.),
                    tr("action.restore"),
                    Style::Quiet,
                    enabled,
                )
            {
                self.ask(
                    &i18n::message("action.restore_named", &[("name", name)]),
                    words(&["agents", "revert", name]),
                );
            }
        }
    }
    pub(super) fn plugins(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let enabled = self.enabled();
        if self.ui.button(
            c,
            750,
            Rect::new(x + width - 130., 35., 130., 42.),
            tr("action.install_plugin"),
            Style::Primary,
            enabled,
        ) {
            self.plugin = PluginForm::default();
            self.plugin_panel = 2;
        }
        let items = self.state.get("plugins").get("plugins").array().to_vec();
        let rows = ((c.height - 214.) / 90.).max(1.) as usize;
        self.scroll = self.scroll.min(items.len().saturating_sub(rows));
        if items.is_empty() {
            empty(
                c,
                Rect::new(x, 140., width, 172.),
                tr("plugins.empty"),
                tr("plugins.empty_help"),
            );
        }
        for (index, item) in items.iter().enumerate().skip(self.scroll).take(rows) {
            let y = 140. + (index - self.scroll) as f32 * 90.;
            let r = Rect::new(x, y, width, 77.);
            card(c, r);
            if self
                .ui
                .button(c, 30000 + index as u64, r, "", Style::Quiet, enabled)
            {
                self.selected_plugin = item.clone();
                self.plugin.name = item.get("name").text().into();
                self.plugin.revision.clear();
                self.plugin_panel = 1;
            }
            c.ellipsis(
                value(item, "name", "plugin"),
                x + 22.,
                y + 13.,
                width - 170.,
                18,
                true,
                TEXT,
            );
            c.text(
                &format!(
                    "{} · {}",
                    if item.get("kind").text() == "native" {
                        tr("plugins.native")
                    } else {
                        tr("plugins.community")
                    },
                    value(item, "version", tr("plugins.unversioned"))
                ),
                x + 22.,
                y + 45.,
                13,
                false,
                MUTED,
            );
            let on = item.get("enabled").boolean();
            c.text(
                if on {
                    tr("status.enabled")
                } else {
                    tr("status.disabled")
                },
                x + width - 100.,
                y + 28.,
                13,
                false,
                if on { GREEN } else { MUTED },
            );
            c.text("›", x + width - 31., y + 24., 24, false, MUTED);
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
        let modes = [
            ("off", tr("privacy.off"), tr("privacy.off_help")),
            ("secrets", tr("privacy.secrets"), tr("privacy.secrets_help")),
            (
                "personal",
                tr("privacy.personal"),
                tr("privacy.personal_help"),
            ),
        ];
        for (index, (id, title, description)) in modes.iter().enumerate() {
            let y = 139. + index as f32 * 115.;
            let selected = mode == *id;
            let r = Rect::new(x, y, width, 99.);
            c.rect(r, 16., if selected { 0xe8f0feff } else { LINE });
            c.rect(
                r.inset(1.),
                15.,
                if selected { 0xf5f8ffff } else { SURFACE },
            );
            if !selected
                && self
                    .ui
                    .button(c, 800 + index as u64, r, "", Style::Quiet, enabled)
            {
                self.action("Set privacy mode", words(&["privacy", "set", id]), None);
            }
            c.rect(
                Rect::new(x + 24., y + 27., 20., 20.),
                10.,
                if selected { BLUE } else { 0x9aa0a6ff },
            );
            c.rect(Rect::new(x + 26., y + 29., 16., 16.), 8., SURFACE);
            if selected {
                c.rect(Rect::new(x + 30., y + 33., 8., 8.), 4., BLUE);
            }
            c.text(title, x + 62., y + 20., 19, true, TEXT);
            c.text(description, x + 62., y + 57., 14, false, MUTED);
            if selected {
                c.text(
                    tr("privacy.saved"),
                    x + width - 56.,
                    y + 27.,
                    12,
                    true,
                    BLUE,
                );
            }
        }
        let pending = self.privacy_needs_restart();
        let runtime_notice = self.privacy_runtime_notice();
        let boundary_y = if runtime_notice.is_some() { 570. } else { 510. };
        if let Some(notice) = runtime_notice {
            c.ellipsis(
                notice,
                x + 2.,
                510.,
                if pending { width - 172. } else { width },
                14,
                false,
                TEXT,
            );
            if pending
                && self.ui.button(
                    c,
                    810,
                    Rect::new(x + width - 154., 496., 154., 40.),
                    tr("service.title"),
                    Style::Quiet,
                    enabled,
                )
            {
                self.service_panel = true;
            }
        }
        c.text(tr("privacy.boundaries"), x + 2., boundary_y, 17, true, TEXT);
        c.text(
            tr("privacy.boundaries_help"),
            x + 2.,
            boundary_y + 34.,
            14,
            false,
            MUTED,
        );
        if state.get("model_auto").boolean() {
            c.text(
                tr("privacy.model_auto"),
                x + 2.,
                boundary_y + 70.,
                13,
                false,
                MUTED,
            );
        }
    }
    pub(super) fn updates(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let enabled = self.enabled();
        let items = [
            (
                tr("updates.gateway"),
                value(self.state.get("status"), "version", tr("status.not_found")).to_owned(),
                self.gateway_update.clone(),
            ),
            (
                tr("updates.gui"),
                VERSION.to_owned(),
                self.gui_update.clone(),
            ),
        ];
        for (index, (title, current, info)) in items.iter().enumerate() {
            let y = 140. + index as f32 * 162.;
            card(c, Rect::new(x, y, width, 140.));
            c.text(title, x + 24., y + 21., 20, true, TEXT);
            c.text(
                &i18n::message("updates.current", &[("current", current)]),
                x + 24.,
                y + 62.,
                15,
                false,
                MUTED,
            );
            let latest = value(info, "latest", value(info, "latest_version", ""));
            if !latest.is_empty() {
                c.text(
                    &i18n::message("updates.latest", &[("latest", latest)]),
                    x + 24.,
                    y + 96.,
                    13,
                    false,
                    MUTED,
                );
            }
            let update = info.get("available").boolean();
            let managed = info.get("managed").boolean();
            let label = if managed {
                tr("action.package_update")
            } else if update {
                tr("action.update_now")
            } else {
                tr("action.check_update")
            };
            let width_button = if managed { 166. } else { 122. };
            if self.ui.button(
                c,
                850 + index as u64,
                Rect::new(x + width - width_button - 24., y + 48., width_button, 42.),
                label,
                if update && !managed {
                    Style::Primary
                } else {
                    Style::Secondary
                },
                enabled,
            ) {
                if managed {
                    self.package_update(index == 1);
                } else if index == 0 {
                    if update {
                        self.ask(tr("action.update_gateway"), words(&["update", "apply"]));
                    } else {
                        self.action("Check gateway update", words(&["update", "check"]), None);
                    }
                } else {
                    self.self_update(update);
                }
            }
        }
        c.text(tr("updates.verify"), x + 2., 505., 14, false, MUTED);
        c.text(tr("updates.managed_help"), x + 2., 537., 14, false, MUTED);
    }
}
