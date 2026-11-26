use crate::provider_oauth::{self, LMM_DOMAIN, LMM_PLUGIN, LMM_REVISION, LMM_SOURCE};
use crate::view_state::GatewayState;
use crate::*;
pub(super) fn sheet(c: &mut Canvas<'_>, heading: &str) -> Rect {
    c.rect(Rect::new(0., 0., c.width, c.height), 0., 0x00000038);
    let r = Rect::new((c.width - 468.).max(0.), 0., 468., c.height);
    c.rect(r, 0., SURFACE);
    c.text(heading, r.x + 32., 38., 17, true, TEXT);
    r
}
impl App {
    pub(super) fn clear_key(&mut self) {
        let mut bytes = std::mem::take(&mut self.provider.key).into_bytes();
        bytes.fill(0);
    }
    fn drawer_notice(&self, c: &mut Canvas<'_>, r: Rect) {
        if self.failed {
            c.ellipsis(
                &self.notice,
                r.x + 32.,
                r.h - 150.,
                r.w - 64.,
                13,
                false,
                RED,
            );
        } else if self.busy {
            c.text(
                tr("status.processing"),
                r.x + 32.,
                r.h - 150.,
                14,
                false,
                MUTED,
            );
        }
    }
    pub(super) fn provider_dialog(&mut self, c: &mut Canvas<'_>) {
        let r = sheet(
            c,
            if self.provider_panel == 1 {
                tr("providers.details")
            } else if self.provider_panel == 4 {
                tr("providers.advanced")
            } else if self.provider_edit {
                tr("providers.edit")
            } else {
                tr("action.add_provider")
            },
        );
        let x = r.x + 32.;
        let width = r.w - 64.;
        let enabled = !self.busy && self.confirm.is_none() && self.plugin_panel == 0;
        if self.ui.button(
            c,
            9100,
            Rect::new(r.x + r.w - 66., 25., 40., 40.),
            "×",
            Style::Quiet,
            enabled,
        ) {
            self.provider_panel = 0;
            self.protocol_menu = false;
            self.clear_key();
            return;
        }
        if self.provider.oauth {
            self.oauth_provider_dialog(c, r, enabled);
            return;
        }
        if self.provider_panel == 4 {
            self.provider_advanced(c, r, enabled);
            return;
        }
        if self.provider_panel == 1 {
            c.ellipsis(&self.provider.name, x, 100., width, 27, true, TEXT);
            c.text(tr("providers.url"), x, 166., 13, true, MUTED);
            c.ellipsis(&self.provider.url, x, 195., width, 16, false, TEXT);
            c.text(tr("providers.protocol"), x, 245., 13, true, MUTED);
            c.text(
                protocol_name(self.provider.protocol),
                x,
                275.,
                17,
                false,
                TEXT,
            );
            if self.ui.button(
                c,
                9101,
                Rect::new(x, 336., 126., 42.),
                tr("action.edit"),
                Style::Primary,
                enabled,
            ) {
                self.provider_panel = 2;
            }
            if self.ui.button(
                c,
                9102,
                Rect::new(x + 142., 336., 142., 42.),
                tr("action.test_provider"),
                Style::Secondary,
                enabled,
            ) {
                let name = self.provider.name.clone();
                self.action("Test provider", words(&["providers", "test", &name]), None);
            }
            if self.ui.button(
                c,
                9104,
                Rect::new(x, 390., 180., 40.),
                tr("providers.browse_models"),
                Style::Secondary,
                enabled,
            ) {
                self.open_provider_models(self.provider.name.clone(), self.provider.name.clone());
                return;
            }
            c.text(
                tr("providers.remove_title"),
                x,
                (r.h - 181.).max(474.),
                14,
                true,
                TEXT,
            );
            c.text(
                tr("providers.remove_help"),
                x,
                (r.h - 152.).max(500.),
                13,
                false,
                MUTED,
            );
            if self.ui.button(
                c,
                9103,
                Rect::new(x, (r.h - 101.).max(532.), 140., 40.),
                tr("action.remove_provider"),
                Style::Danger,
                enabled,
            ) {
                let name = self.provider.name.clone();
                self.ask(
                    tr("action.remove_provider"),
                    words(&["providers", "remove", &name]),
                );
            }
            if self.failed {
                c.ellipsis(&self.notice, x, 439., width, 13, false, RED);
            } else if self.busy {
                c.text(tr("status.testing"), x, 439., 14, false, MUTED);
            }
        } else {
            let basics = self.provider_panel == 2;
            c.text(
                if basics {
                    tr("providers.basics")
                } else {
                    tr("providers.access")
                },
                x,
                100.,
                27,
                true,
                TEXT,
            );
            c.text(
                if basics {
                    tr("providers.step1")
                } else {
                    tr("providers.step2")
                },
                x,
                146.,
                13,
                false,
                MUTED,
            );
            if basics {
                if !self.provider_edit
                    && self.ui.button(
                        c,
                        9111,
                        Rect::new(x + width - 156., 132., 156., 38.),
                        tr("providers.oauth_mode"),
                        Style::Secondary,
                        enabled,
                    )
                {
                    self.provider.oauth = true;
                    self.provider.name.clear();
                    self.provider.protocol = 0;
                    self.provider.url = LMM_DOMAIN.into();
                    self.repaint = true;
                    self.provider.plugin_id = "lmm".into();
                    self.provider.oauth_spec.clear();
                    self.provider.endpoint_menu = false;
                    self.protocol_menu = false;
                    self.open_oauth_market();
                    return;
                }
                field(
                    &mut self.ui,
                    c,
                    9120,
                    x,
                    196.,
                    width,
                    tr("field.name"),
                    &mut self.provider.name,
                    tr("providers.name_hint"),
                    false,
                    enabled && !self.provider_edit && !self.protocol_menu,
                );
                self.provider_endpoint(c, x, 296., width, enabled && !self.protocol_menu);
                if !self.provider.endpoint_menu {
                    c.text(tr("providers.protocol"), x, 396., 14, true, TEXT);
                    if self.ui.button(
                        c,
                        9122,
                        Rect::new(x, 425., width, 44.),
                        &format!("{}  ▾", protocol_name(self.provider.protocol)),
                        Style::Secondary,
                        enabled,
                    ) {
                        self.protocol_menu = !self.protocol_menu;
                    }
                }
            } else {
                field(
                    &mut self.ui,
                    c,
                    9130,
                    x,
                    196.,
                    width,
                    tr("providers.key"),
                    &mut self.provider.key,
                    if self.provider_edit {
                        tr("providers.keep_key")
                    } else {
                        tr("providers.optional_key")
                    },
                    true,
                    enabled,
                );
                c.text(tr("providers.key_help"), x, 284., 13, false, MUTED);
                field(
                    &mut self.ui,
                    c,
                    9131,
                    x,
                    342.,
                    width,
                    tr("providers.model"),
                    &mut self.provider.model,
                    tr("providers.model_hint"),
                    false,
                    enabled,
                );
            }
            if !basics
                && self.ui.button(
                    c,
                    9140,
                    Rect::new(x, 441., width, 37.),
                    tr("providers.advanced"),
                    Style::Secondary,
                    enabled,
                )
            {
                self.provider_panel = 4;
                self.ui = Ui::default();
                self.repaint = true;
                return;
            }
            self.drawer_notice(c, r);
            if self.ui.button(
                c,
                9132,
                Rect::new(x, r.h - 87., 110., 44.),
                if basics {
                    tr("action.cancel")
                } else {
                    tr("action.back")
                },
                Style::Quiet,
                enabled && !self.protocol_menu,
            ) {
                if basics {
                    self.provider_panel = 0;
                    self.clear_key();
                } else {
                    self.provider_panel = 2;
                }
            }
            let valid = !self.provider.name.trim().is_empty()
                && (self.provider.url.trim().starts_with("https://")
                    || self.provider.url.trim().starts_with("http://"));
            if self.ui.button(
                c,
                9133,
                Rect::new(r.x + r.w - 174., r.h - 87., 142., 44.),
                if basics {
                    tr("action.next")
                } else {
                    tr("action.save_provider")
                },
                Style::Primary,
                enabled && valid && !self.protocol_menu,
            ) {
                if basics {
                    self.provider.endpoint_menu = false;
                    self.provider_panel = 3;
                } else {
                    let mut args = words(&[
                        "providers",
                        if self.provider_edit { "edit" } else { "add" },
                        self.provider.name.trim(),
                        "--base-url",
                        self.provider.url.trim(),
                        "--protocol",
                        protocols()[self.provider.protocol],
                    ]);
                    if !self.provider.model.trim().is_empty() {
                        args.extend(words(&["--model", self.provider.model.trim()]));
                        if !self.provider.route.trim().is_empty() {
                            args.extend(words(&["--route", self.provider.route.trim()]));
                        }
                    }
                    if self.provider.authentication != self.provider.original_authentication {
                        args.extend(words(&[
                            "--auth",
                            PROVIDER_AUTH[self.provider.authentication],
                        ]));
                    }
                    if self.provider.header_refs != self.provider.original_header_refs {
                        args.extend(words(&["--header-env", self.provider.header_refs.trim()]));
                    }
                    if self.provider.key.is_empty() && !self.provider.environment.is_empty() {
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
            if self.protocol_menu {
                let count = protocols().len();
                let height = count as f32 * 34. + 12.;
                let y = (r.h - height - 55.).min(474.);
                card(c, Rect::new(x, y, width, height));
                for i in 0..count {
                    let b = Rect::new(x + 5., y + 5. + i as f32 * 34., width - 10., 34.);
                    if self
                        .ui
                        .button(c, 9150 + i as u64, b, "", Style::Quiet, enabled)
                    {
                        self.provider.protocol = i;
                        self.protocol_menu = false;
                    }
                    c.text(
                        protocol_name(i),
                        x + 18.,
                        b.y + 8.,
                        14,
                        false,
                        if i == self.provider.protocol {
                            ACCENT
                        } else {
                            TEXT
                        },
                    );
                }
            }
        }
    }
    fn provider_advanced(&mut self, c: &mut Canvas<'_>, r: Rect, enabled: bool) {
        let x = r.x + 32.;
        let width = r.w - 64.;
        let supported = self
            .state
            .get("providers")
            .get("advanced_options")
            .boolean();
        c.text(tr("providers.advanced"), x, 96., 25, true, TEXT);
        field(
            &mut self.ui,
            c,
            9170,
            x,
            144.,
            width,
            tr("providers.auth_env"),
            &mut self.provider.environment,
            "PROVIDER_API_KEY",
            false,
            enabled,
        );
        field(
            &mut self.ui,
            c,
            9171,
            x,
            230.,
            width,
            tr("providers.route_alias"),
            &mut self.provider.route,
            "",
            false,
            enabled,
        );
        c.text(tr("providers.auth_method"), x, 316., 14, true, TEXT);
        let names = [
            tr("settings.system"),
            "Bearer",
            "x-api-key",
            "x-goog-api-key",
            tr("providers.auth_none"),
        ];
        let column = (width - 12.) / 3.;
        for (index, name) in names.into_iter().enumerate() {
            if self.ui.button(
                c,
                9172 + index as u64,
                Rect::new(
                    x + (index % 3) as f32 * (column + 6.),
                    339. + (index / 3) as f32 * 32.,
                    column,
                    29.,
                ),
                name,
                if self.provider.authentication == index {
                    Style::Primary
                } else {
                    Style::Secondary
                },
                enabled && supported,
            ) {
                self.provider.authentication = index;
                self.repaint = true;
            }
        }
        field(
            &mut self.ui,
            c,
            9177,
            x,
            413.,
            width,
            tr("providers.header_refs"),
            &mut self.provider.header_refs,
            "X-Client=CLIENT_ID, X-Project=PROJECT_ID",
            false,
            enabled && supported,
        );
        if !supported {
            c.ellipsis(
                tr("providers.advanced_upgrade"),
                x,
                495.,
                width,
                13,
                false,
                MUTED,
            );
        }
        if self.ui.button(
            c,
            9178,
            Rect::new(x, (r.h - 62.).max(520.), 136., 40.),
            tr("action.back"),
            Style::Primary,
            enabled,
        ) {
            self.provider_panel = 3;
            self.ui = Ui::default();
            self.repaint = true;
        }
    }
    fn provider_endpoint(&mut self, c: &mut Canvas<'_>, x: f32, y: f32, width: f32, enabled: bool) {
        let old = self.provider.url.clone();
        field(
            &mut self.ui,
            c,
            9121,
            x,
            y,
            width - 48.,
            tr(if self.provider.oauth {
                "providers.oauth_domain"
            } else {
                "providers.url"
            }),
            &mut self.provider.url,
            if self.provider.oauth {
                if self.provider.plugin_id == "lmm" {
                    LMM_DOMAIN
                } else if self.provider.plugin_endpoint.is_empty() {
                    tr("providers.oauth_plugin_endpoint")
                } else {
                    &self.provider.plugin_endpoint
                }
            } else {
                tr("providers.endpoint_hint")
            },
            false,
            enabled,
        );
        if self.provider.url != old {
            self.repaint = true;
            self.provider.endpoint_menu = true;
            self.provider.endpoint_filter = true;
        }
        if self.ui.button(
            c,
            9124,
            Rect::new(x + width - 44., y + 28., 44., 44.),
            if self.provider.endpoint_menu {
                "▴"
            } else {
                "▾"
            },
            Style::Secondary,
            enabled,
        ) {
            self.repaint = true;
            self.provider.endpoint_menu = !self.provider.endpoint_menu;
            self.provider.endpoint_filter = false;
        }
        if !self.provider.endpoint_menu {
            return;
        }
        let rows = [
            ("LMM", "https://api.lmm.best/v1", LMM_DOMAIN, "lmm", 0),
            (
                "OpenAI",
                "https://api.openai.com/v1",
                "api.openai.com",
                "",
                0,
            ),
            (
                "Anthropic",
                "https://api.anthropic.com/v1",
                "api.anthropic.com",
                "",
                2,
            ),
            (
                "OpenRouter",
                "https://openrouter.ai/api/v1",
                "openrouter.ai",
                "",
                0,
            ),
            (
                "DeepSeek",
                "https://api.deepseek.com/v1",
                "api.deepseek.com",
                "",
                0,
            ),
            (
                "Google Gemini",
                "https://generativelanguage.googleapis.com/v1beta",
                "generativelanguage.googleapis.com",
                "",
                3,
            ),
        ];
        let mut shown = 0;
        for (index, (name, url, host, plugin, protocol)) in rows.into_iter().enumerate() {
            if self.provider.oauth && plugin.is_empty() {
                continue;
            }
            let candidate = format!("{name} {url}");
            if self.provider.endpoint_filter
                && !provider_oauth::endpoint_matches(&self.provider.url, &candidate)
            {
                continue;
            }
            let row = Rect::new(x, y + 76. + shown as f32 * 29., width, 29.);
            shown += 1;
            if self
                .ui
                .button(c, 9160 + index as u64, row, "", Style::Secondary, enabled)
            {
                self.provider.url = if self.provider.oauth { host } else { url }.into();
                if self.provider.oauth {
                    self.provider.plugin_id = plugin.into();
                    self.provider.oauth_method = None;
                    self.provider.oauth_spec.clear();
                    self.provider.plugin_endpoint.clear();
                    self.provider.pending_oauth_binding = Json::Null;
                } else if self.provider.name.trim().is_empty() && !self.provider_edit {
                    self.provider.name = [
                        "lmm",
                        "openai",
                        "anthropic",
                        "openrouter",
                        "deepseek",
                        "gemini",
                    ][index]
                        .into();
                }
                self.provider.protocol = protocol;
                self.provider.endpoint_menu = false;
                self.repaint = true;
            }
            c.ellipsis(
                &format!(
                    "{name}  ·  {}",
                    if self.provider.oauth { host } else { url }
                ),
                row.x + 10.,
                row.y + 6.,
                row.w - 20.,
                13,
                false,
                TEXT,
            );
        }
        if shown == 0 {
            c.text(
                tr("providers.endpoint_no_matches"),
                x,
                y + 83.,
                13,
                false,
                MUTED,
            );
        }
    }

    fn open_oauth_market(&mut self) {
        self.provider.plugin_picker = true;
        self.provider.browse_market = true;
        self.provider.plugin_offset = 0;
        self.provider.plugin_query.clear();
        self.repaint = true;
        self.action(
            "Magpie provider market",
            words(&["plugins", "provider-market"]),
            None,
        );
    }

    fn oauth_provider_dialog(&mut self, c: &mut Canvas<'_>, r: Rect, enabled: bool) {
        let x = r.x + 32.;
        let width = r.w - 64.;
        c.text(tr("providers.oauth_mode"), x, 100., 27, true, TEXT);
        if self.provider.plugin_picker {
            self.oauth_plugin_picker(c, r, enabled);
            return;
        }
        if self.busy {
            if self.oauth_prompt != Json::Null {
                self.oauth_plugin_prompt(c, r);
                return;
            }
            if self.oauth_instructions.is_empty() {
                c.text(tr("providers.oauth_wait"), x, 161., 14, false, MUTED);
            } else {
                c.ellipsis(&self.oauth_instructions, x, 159., width, 13, false, MUTED);
                if !self.oauth_expects_code {
                    if self.ui.button(
                        c,
                        9180,
                        Rect::new(x, 203., 170., 40.),
                        tr("providers.oauth_copy_instructions"),
                        Style::Secondary,
                        true,
                    ) {
                        self.ui.clipboard = Some(self.oauth_instructions.clone());
                    }
                    return;
                }
                field(
                    &mut self.ui,
                    c,
                    9170,
                    x,
                    198.,
                    width,
                    tr("providers.oauth_code"),
                    &mut self.oauth_code,
                    "",
                    true,
                    true,
                );
                if self.ui.button(
                    c,
                    9171,
                    Rect::new(x, r.h - 87., 120., 44.),
                    tr("action.cancel"),
                    Style::Quiet,
                    true,
                ) {
                    self.oauth_input = None;
                    self.oauth_instructions.clear();
                    self.repaint = true;
                }
                if self.ui.button(
                    c,
                    9172,
                    Rect::new(x + width - 142., r.h - 87., 142., 44.),
                    tr("action.next"),
                    Style::Primary,
                    !self.oauth_code.trim().is_empty(),
                ) {
                    if let Some(input) = self.oauth_input.take() {
                        let _ = input.send(std::mem::take(&mut self.oauth_code));
                    }
                    self.oauth_instructions.clear();
                    self.repaint = true;
                }
            }
            return;
        }
        self.provider_endpoint(c, x, 153., width, enabled);
        if self.ui.button(
            c,
            9179,
            Rect::new(x, 304., width, 40.),
            if self.provider.plugin_id == "lmm" {
                tr("providers.oauth_other_plugin")
            } else {
                &self.provider.plugin_id
            },
            Style::Secondary,
            enabled,
        ) {
            self.open_oauth_market();
        }
        let lmm = self.provider.plugin_id.trim() == "lmm";
        let installed = self
            .state
            .get("plugins")
            .get("plugins")
            .array()
            .iter()
            .find(|plugin| plugin.get("name").text() == LMM_PLUGIN)
            .map(|plugin| plugin.get("enabled").boolean());
        if lmm && installed.is_none() {
            if self.ui.button(
                c,
                9174,
                Rect::new(x, 367., width, 42.),
                tr("providers.oauth_install_lmm"),
                Style::Secondary,
                enabled,
            ) {
                self.ask(
                    tr("providers.oauth_install_lmm"),
                    words(&[
                        "plugins",
                        "install",
                        LMM_SOURCE,
                        "--revision",
                        LMM_REVISION,
                        "--name",
                        LMM_PLUGIN,
                    ]),
                );
            }
        } else if lmm && installed == Some(false) {
            if self.ui.button(
                c,
                9175,
                Rect::new(x, 367., width, 42.),
                tr("action.enable_plugin"),
                Style::Secondary,
                enabled,
            ) {
                self.ask(
                    tr("action.enable_plugin"),
                    words(&["plugins", "enable", LMM_PLUGIN]),
                );
            }
        } else if self.ui.button(
            c,
            9176,
            Rect::new(x, 367., width, 42.),
            tr("action.install_plugin"),
            Style::Secondary,
            enabled,
        ) {
            self.plugin = PluginForm::default();
            self.plugin_panel = 2;
            return;
        }
        self.drawer_notice(c, r);
        if self.ui.button(
            c,
            9177,
            Rect::new(x, r.h - 87., 140., 44.),
            tr("providers.api_key_mode"),
            Style::Quiet,
            enabled,
        ) {
            self.provider.oauth = false;
            self.provider.url.clear();
            self.repaint = true;
            self.provider.endpoint_menu = false;
        }
        let pending = self.provider.pending_oauth_binding != Json::Null;
        let ready = (!self.provider.url.trim().is_empty()
            || (!pending && self.provider.oauth_method.is_some()))
            && !self.provider.plugin_id.trim().is_empty()
            && (!lmm || installed == Some(true));
        if self.ui.button(
            c,
            9178,
            Rect::new(x + width - 172., r.h - 87., 172., 44.),
            tr(if pending {
                "action.save_provider"
            } else {
                "providers.oauth_login"
            }),
            Style::Primary,
            enabled && ready,
        ) {
            self.start_oauth();
        }
    }
    fn oauth_plugin_prompt(&mut self, c: &mut Canvas<'_>, r: Rect) {
        let x = r.x + 32.;
        let width = r.w - 64.;
        let prompt = &self.oauth_prompt;
        let is_select = prompt.get("type").text() == "select";
        let mut selected = None;
        if is_select {
            c.ellipsis(prompt.get("message").text(), x, 155., width, 14, true, TEXT);
            let options = prompt.get("options").array();
            let count = ((r.h - 322.) / 43.).floor().clamp(1., 6.) as usize;
            let offset = self.oauth_prompt_offset;
            for (index, option) in options.iter().skip(offset).take(count).enumerate() {
                let row = Rect::new(x, 195. + index as f32 * 43., width, 39.);
                if self
                    .ui
                    .button(c, 9240 + index as u64, row, "", Style::Secondary, true)
                {
                    selected = Some(option.get("value").text().to_owned());
                }
                c.ellipsis(
                    option.get("label").text(),
                    row.x + 10.,
                    row.y + 11.,
                    row.w - 20.,
                    14,
                    false,
                    TEXT,
                );
            }
            if options.len() > count
                && self.ui.button(
                    c,
                    9247,
                    Rect::new(x, r.h - 145., 48., 37.),
                    "‹",
                    Style::Quiet,
                    offset > 0,
                )
            {
                self.oauth_prompt_offset = offset.saturating_sub(count);
                self.repaint = true;
            }
            if options.len() > count
                && self.ui.button(
                    c,
                    9248,
                    Rect::new(x + 52., r.h - 145., 48., 37.),
                    "›",
                    Style::Quiet,
                    offset + count < options.len(),
                )
            {
                self.oauth_prompt_offset += count;
                self.repaint = true;
            }
        } else {
            c.ellipsis(prompt.get("message").text(), x, 155., width, 14, true, TEXT);
            self.ui.input(
                c,
                9250,
                Rect::new(x, 187., width, 46.),
                &mut self.oauth_code,
                prompt.get("placeholder").text(),
                true,
                true,
            );
            if self.ui.button(
                c,
                9251,
                Rect::new(x + width - 142., r.h - 87., 142., 44.),
                tr("action.next"),
                Style::Primary,
                true,
            ) {
                selected = Some(std::mem::take(&mut self.oauth_code));
            }
        }
        if prompt.get("rejected").boolean() {
            c.text(
                tr("providers.oauth_prompt_invalid"),
                x,
                r.h - 182.,
                13,
                false,
                RED,
            );
        }
        if self.ui.button(
            c,
            9252,
            Rect::new(x, r.h - 87., 110., 44.),
            tr("action.cancel"),
            Style::Quiet,
            true,
        ) {
            self.oauth_input = None;
            self.oauth_prompt = Json::Null;
            self.oauth_code.clear();
            self.repaint = true;
        } else if let Some(answer) = selected {
            if let Some(input) = &self.oauth_input {
                let _ = input.send(answer);
            }
            self.oauth_prompt = Json::Null;
            self.repaint = true;
        }
    }
    fn oauth_plugin_picker(&mut self, c: &mut Canvas<'_>, r: Rect, enabled: bool) {
        let x = r.x + 32.;
        let width = r.w - 64.;
        if self.provider.browse_market {
            self.oauth_magpie_market(c, r, enabled);
            return;
        }
        let old = self.provider.plugin_query.clone();
        field(
            &mut self.ui,
            c,
            9181,
            x,
            155.,
            width,
            tr("providers.oauth_choose_plugin"),
            &mut self.provider.plugin_query,
            tr("providers.oauth_search"),
            false,
            enabled,
        );
        if self.provider.plugin_query != old {
            self.provider.plugin_offset = 0;
            self.repaint = true;
        }
        let options = provider_oauth::plugin_methods(
            &self.provider.plugin_catalog,
            &self.provider.plugin_query,
        );
        if options.is_empty() && !self.busy && !self.failed {
            c.text(tr("providers.oauth_no_plugins"), x, 250., 13, false, MUTED);
        }
        let page_size = ((r.h - 390.) / 43.).floor().clamp(1., 6.) as usize;
        let offset = self.provider.plugin_offset;
        for (index, option) in options.iter().skip(offset).take(page_size).enumerate() {
            let row = Rect::new(x, 244. + index as f32 * 43., width, 39.);
            if self
                .ui
                .button(c, 9200 + index as u64, row, "", Style::Secondary, enabled)
            {
                self.provider.plugin_id = option.provider.into();
                self.provider.oauth_method = Some(option.method);
                self.provider.oauth_spec = option.spec.into();
                self.provider.plugin_endpoint = option.endpoint.into();
                self.provider.pending_oauth_binding = Json::Null;
                self.provider.url = if option.provider == "lmm" {
                    LMM_DOMAIN.into()
                } else {
                    String::new()
                };
                self.provider.protocol = option.protocol;
                self.provider.endpoint_menu = false;
                self.provider.plugin_picker = false;
                self.repaint = true;
                return;
            }
            let label = format!("{} · {}", option.name, option.label);
            c.ellipsis(
                &label,
                row.x + 10.,
                row.y + 11.,
                row.w - 20.,
                14,
                false,
                TEXT,
            );
        }
        if options.len() > page_size
            && self.ui.button(
                c,
                9182,
                Rect::new(x, r.h - 154., 48., 37.),
                "‹",
                Style::Quiet,
                enabled && offset > 0,
            )
        {
            self.provider.plugin_offset = offset.saturating_sub(page_size);
            self.repaint = true;
        }
        if options.len() > page_size
            && self.ui.button(
                c,
                9183,
                Rect::new(x + 52., r.h - 154., 48., 37.),
                "›",
                Style::Quiet,
                enabled && offset + page_size < options.len(),
            )
        {
            self.provider.plugin_offset += page_size;
            self.repaint = true;
        }
        self.drawer_notice(c, Rect { h: r.h + 40., ..r });
        if self.ui.button(
            c,
            9184,
            Rect::new(x, r.h - 87., 110., 44.),
            tr("action.back"),
            Style::Quiet,
            enabled,
        ) {
            self.provider.plugin_picker = false;
            self.repaint = true;
        }
        if self.ui.button(
            c,
            9186,
            Rect::new(x + width - 180., r.h - 151., 180., 38.),
            tr("providers.oauth_magpie_market"),
            Style::Secondary,
            enabled,
        ) {
            self.provider.browse_market = true;
            self.provider.plugin_query.clear();
            self.provider.plugin_offset = 0;
            self.repaint = true;
            self.action(
                "Magpie provider market",
                words(&["plugins", "provider-market"]),
                None,
            );
        }
        if self.ui.button(
            c,
            9185,
            Rect::new(x + width - 165., r.h - 87., 165., 44.),
            tr("action.install_plugin"),
            Style::Primary,
            enabled,
        ) {
            self.provider.plugin_picker = false;
            self.plugin = PluginForm::default();
            self.plugin_panel = 2;
            self.repaint = true;
        }
    }
    fn oauth_magpie_market(&mut self, c: &mut Canvas<'_>, r: Rect, enabled: bool) {
        let x = r.x + 32.;
        let width = r.w - 64.;
        let previous = self.provider.plugin_query.clone();
        field(
            &mut self.ui,
            c,
            9300,
            x,
            155.,
            width,
            tr("providers.oauth_magpie_market"),
            &mut self.provider.plugin_query,
            tr("providers.oauth_search"),
            false,
            enabled,
        );
        if self.provider.plugin_query != previous {
            self.provider.plugin_offset = 0;
            self.repaint = true;
        }
        let search = &self.provider.plugin_query;
        let subscriptions = provider_oauth::subscription_listings().collect::<Vec<_>>();
        let mut listings =
            subscriptions
                .iter()
                .chain(self.provider.plugin_market.get("plugins").array())
                .filter(|item| {
                    let candidate =
                        format!("{} {}", item.get("name").text(), item.get("package").text(),);
                    provider_oauth::endpoint_matches(search, &candidate)
                        || item.get("providers").array().iter().any(|provider| {
                            provider_oauth::endpoint_matches(search, provider.text())
                        })
                })
                .collect::<Vec<_>>();
        listings.sort_by_key(|item| provider_oauth::market_priority(item.get("package").text()));
        let offset = self.provider.plugin_offset;
        let count = ((r.h - 390.) / 43.).floor().clamp(1., 6.) as usize;
        for (index, item) in listings.iter().skip(offset).take(count).enumerate() {
            let pkg = item.get("package").text();
            let rect = Rect::new(x, 244. + index as f32 * 43., width, 39.);
            let installed = self
                .state
                .get("plugins")
                .get("plugins")
                .array()
                .iter()
                .find(|plugin| {
                    let source = plugin.get("source").text();
                    source
                        .strip_prefix("npm:")
                        .or_else(|| source.strip_prefix("magpie:"))
                        .and_then(|spec| spec.split('#').next())
                        .and_then(|spec| spec.rsplit_once('@'))
                        .is_some_and(|(name, _)| name == pkg)
                });
            let label = match installed {
                Some(plugin) if plugin.get("enabled").boolean() => {
                    format!("{} · {}", item.get("name").text(), tr("plugins.enabled"))
                }
                Some(_) => format!(
                    "{} · {}",
                    item.get("name").text(),
                    tr("action.enable_plugin")
                ),
                None => format!(
                    "{} · {}",
                    item.get("name").text(),
                    tr("action.install_plugin")
                ),
            };
            if self
                .ui
                .button(c, 9310 + index as u64, rect, "", Style::Secondary, enabled)
            {
                if let Some(plugin) = installed {
                    if plugin.get("enabled").boolean() {
                        self.provider.browse_market = false;
                        self.provider.plugin_query = item
                            .get("providers")
                            .array()
                            .first()
                            .map_or_else(String::new, |provider| provider.text().to_owned());
                        self.provider.plugin_offset = 0;
                        self.action(
                            "Discover OAuth plugins",
                            words(&["plugins", "providers"]),
                            None,
                        );
                    } else {
                        let args = words(&["plugins", "enable", plugin.get("name").text()]);
                        self.ask(tr("action.enable_plugin"), args);
                    }
                } else if !item.get("source").text().is_empty() {
                    let source = item.get("source").text();
                    let title = format!("{} · {source}", tr("action.install_plugin"));
                    self.ask(
                        &title,
                        words(&["plugins", "install", source, "--name", pkg]),
                    );
                } else {
                    let args = words(&["plugins", "provider-market", pkg]);
                    self.action("Resolve Magpie plugin", args, None);
                }
                self.repaint = true;
                return;
            }
            c.ellipsis(
                &label,
                rect.x + 10.,
                rect.y + 11.,
                rect.w - 20.,
                14,
                false,
                TEXT,
            );
        }
        let nav_y = r.h - 143.;
        if self.ui.button(
            c,
            9323,
            Rect::new(x + width - 178., r.h - 87., 178., 44.),
            LMM_DOMAIN,
            Style::Secondary,
            enabled,
        ) {
            self.provider.plugin_picker = false;
            self.provider.browse_market = false;
            self.provider.plugin_id = "lmm".into();
            self.provider.url = LMM_DOMAIN.into();
            self.provider.oauth_method = None;
            self.provider.oauth_spec.clear();
            self.provider.plugin_endpoint.clear();
            self.provider.pending_oauth_binding = Json::Null;
            self.provider.protocol = 0;
            self.provider.endpoint_menu = false;
            self.repaint = true;
        }
        if self.ui.button(
            c,
            9320,
            Rect::new(x, nav_y, 48., 37.),
            "‹",
            Style::Quiet,
            enabled && offset > 0,
        ) {
            self.provider.plugin_offset = offset.saturating_sub(count);
            self.repaint = true;
        }
        if self.ui.button(
            c,
            9321,
            Rect::new(x + 52., nav_y, 48., 37.),
            "›",
            Style::Quiet,
            enabled && offset + count < listings.len(),
        ) {
            self.provider.plugin_offset += count;
            self.repaint = true;
        }
        self.drawer_notice(c, Rect { h: r.h + 40., ..r });
        if self.ui.button(
            c,
            9322,
            Rect::new(x, r.h - 87., 110., 44.),
            tr("providers.oauth_installed"),
            Style::Quiet,
            enabled,
        ) {
            self.provider.browse_market = false;
            self.provider.plugin_query.clear();
            self.provider.plugin_offset = 0;
            self.repaint = true;
            self.action(
                "Discover OAuth plugins",
                words(&["plugins", "providers"]),
                None,
            );
        }
    }
    pub(super) fn plugin_dialog(&mut self, c: &mut Canvas<'_>) {
        let r = sheet(
            c,
            if self.plugin_panel == 1 {
                tr("plugins.details")
            } else {
                tr("action.install_plugin")
            },
        );
        let x = r.x + 32.;
        let width = r.w - 64.;
        let enabled = !self.busy && self.confirm.is_none();
        if self.ui.button(
            c,
            9200,
            Rect::new(r.x + r.w - 66., 25., 40., 40.),
            "×",
            Style::Quiet,
            enabled,
        ) {
            self.plugin_panel = 0;
            return;
        }
        if self.plugin_panel == 1 {
            let item = self.selected_plugin.clone();
            let name = value(&item, "name", "plugin");
            let on = item.get("enabled").boolean();
            c.ellipsis(name, x, 100., width, 27, true, TEXT);
            c.text(
                if on {
                    tr("status.enabled")
                } else {
                    tr("status.disabled")
                },
                x,
                147.,
                14,
                false,
                if on { SUCCESS } else { MUTED },
            );
            c.text(tr("plugins.path"), x, 194., 13, true, MUTED);
            c.ellipsis(item.get("path").text(), x, 224., width, 13, false, TEXT);
            let title = if on {
                tr("action.disable_plugin")
            } else {
                tr("action.enable_plugin")
            };
            if self.ui.button(
                c,
                9201,
                Rect::new(x, 278., 142., 42.),
                title,
                Style::Primary,
                enabled,
            ) {
                self.ask(
                    title,
                    words(&["plugins", if on { "disable" } else { "enable" }, name]),
                );
            }
            field(
                &mut self.ui,
                c,
                9202,
                x,
                367.,
                width,
                tr("plugins.version"),
                &mut self.plugin.revision,
                tr("plugins.version_hint"),
                false,
                enabled,
            );
            if self.ui.button(
                c,
                9203,
                Rect::new(x, 465., 142., 40.),
                tr("action.update_plugin"),
                Style::Secondary,
                enabled && !self.plugin.revision.is_empty(),
            ) {
                let flag = if item.get("kind").text() == "npm" {
                    "--version"
                } else {
                    "--revision"
                };
                self.ask(
                    tr("action.update_plugin"),
                    words(&["plugins", "update", name, flag, &self.plugin.revision]),
                );
            }
            c.rect(Rect::new(x, r.h - 139., width, 1.), 0., LINE);
            if self.ui.button(
                c,
                9204,
                Rect::new(x, r.h - 101., 142., 40.),
                tr("action.remove_plugin"),
                Style::Danger,
                enabled,
            ) {
                self.ask(
                    tr("action.remove_plugin"),
                    words(&["plugins", "remove", name]),
                );
            }
        } else {
            c.text(tr("plugins.add_feature"), x, 100., 27, true, TEXT);
            c.text(tr("plugins.install_help"), x, 147., 14, false, MUTED);
            field(
                &mut self.ui,
                c,
                9220,
                x,
                196.,
                width,
                tr("plugins.source"),
                &mut self.plugin.source,
                tr("plugins.source_hint"),
                false,
                enabled,
            );
            field(
                &mut self.ui,
                c,
                9221,
                x,
                296.,
                width,
                tr("plugins.commit"),
                &mut self.plugin.revision,
                tr("plugins.commit_hint"),
                false,
                enabled,
            );
            field(
                &mut self.ui,
                c,
                9222,
                x,
                396.,
                width,
                tr("field.optional_name"),
                &mut self.plugin.name,
                tr("plugins.name_hint"),
                false,
                enabled,
            );
            self.drawer_notice(c, r);
            if self.ui.button(
                c,
                9223,
                Rect::new(x, r.h - 87., 110., 44.),
                tr("action.cancel"),
                Style::Quiet,
                enabled,
            ) {
                self.plugin_panel = 0;
            }
            if self.ui.button(
                c,
                9224,
                Rect::new(r.x + r.w - 174., r.h - 87., 142., 44.),
                tr("action.install_plugin"),
                Style::Primary,
                enabled && !self.plugin.source.trim().is_empty(),
            ) {
                let mut args = words(&["plugins", "install", self.plugin.source.trim()]);
                if !self.plugin.revision.trim().is_empty() {
                    args.extend(words(&["--revision", self.plugin.revision.trim()]));
                }
                if !self.plugin.name.trim().is_empty() {
                    args.extend(words(&["--name", self.plugin.name.trim()]));
                }
                if !self.plugin.sha256.trim().is_empty() {
                    args.extend(words(&["--sha256", self.plugin.sha256.trim()]));
                }
                self.ask(tr("action.install_plugin"), args);
            }
        }
    }
    pub(super) fn service_dialog(&mut self, c: &mut Canvas<'_>) {
        let r = sheet(c, tr("service.title"));
        let x = r.x + 32.;
        let gateway = view_state::gateway_state(&self.state);
        let healthy = gateway == GatewayState::Healthy;
        let setup = self.setup_step();
        let enabled = !self.busy && self.confirm.is_none();
        if self.ui.button(
            c,
            9300,
            Rect::new(r.x + r.w - 66., 25., 40., 40.),
            "×",
            Style::Quiet,
            enabled,
        ) {
            self.service_panel = false;
        }
        c.ellipsis(
            match gateway {
                GatewayState::Loading => tr("status.loading"),
                GatewayState::Unavailable => tr("overview.unavailable"),
                GatewayState::Offline => tr("service.disconnected"),
                GatewayState::Healthy => tr("service.connected"),
            },
            x,
            100.,
            r.w - 64.,
            27,
            true,
            TEXT,
        );
        c.ellipsis(
            self.setup_help().unwrap_or(tr("service.independent")),
            x,
            154.,
            r.w - 64.,
            14,
            false,
            MUTED,
        );
        if self.ui.button(
            c,
            9301,
            Rect::new(x, 225., 220., 44.),
            match setup {
                Some(core_install_ui::SetupStep::Initialize) => tr("setup.initialize"),
                Some(core_install_ui::SetupStep::RegisterService) => tr("setup.install_service"),
                Some(core_install_ui::SetupStep::Inspect) => tr("action.retry"),
                _ => match gateway {
                    GatewayState::Loading => tr("status.loading_short"),
                    GatewayState::Unavailable => tr("action.retry"),
                    GatewayState::Offline => tr("action.start_service"),
                    GatewayState::Healthy => tr("action.restart_service"),
                },
            },
            Style::Primary,
            enabled && gateway != GatewayState::Loading,
        ) {
            if let Some(step) = setup.filter(|step| *step != core_install_ui::SetupStep::Ready) {
                self.trigger_setup_step(step);
            } else if gateway == GatewayState::Unavailable {
                self.refresh();
            } else {
                self.ask(
                    if healthy {
                        tr("action.restart_service_confirm")
                    } else {
                        tr("action.start_service_confirm")
                    },
                    words(&["service", if healthy { "restart" } else { "start" }]),
                );
            }
        }
        if healthy
            && self.ui.button(
                c,
                9302,
                Rect::new(x, r.h - 101., 142., 40.),
                tr("action.stop_service"),
                Style::Danger,
                enabled,
            )
        {
            self.ask(
                tr("action.stop_service_confirm"),
                words(&["service", "stop"]),
            );
        }
    }
    pub(super) fn confirm_dialog(&mut self, c: &mut Canvas<'_>) {
        c.rect(Rect::new(0., 0., c.width, c.height), 0., 0x00000066);
        let installer = self.confirm.as_ref().unwrap().install;
        let setup = self.confirm.as_ref().unwrap().setup;
        let height = if installer.is_some() || setup.is_some() {
            278.
        } else {
            230.
        };
        let r = Rect::new(
            ((c.width - 452.) / 2.).round(),
            ((c.height - height) / 2.).round(),
            452.,
            height,
        );
        c.rect(r, 20., SURFACE);
        let label = self.confirm.as_ref().unwrap().label.clone();
        c.ellipsis(&label, r.x + 28., r.y + 30., 396., 23, true, TEXT);
        if installer.is_some() {
            c.ellipsis(
                &i18n::message(
                    "install.confirm_version",
                    &[("version", self.core_installation.get("version").text())],
                ),
                r.x + 28.,
                r.y + 80.,
                396.,
                15,
                false,
                TEXT,
            );
            c.text(
                tr("install.location"),
                r.x + 28.,
                r.y + 111.,
                12,
                true,
                MUTED,
            );
            c.ellipsis(
                self.core_installation.get("prefix").text(),
                r.x + 28.,
                r.y + 131.,
                396.,
                14,
                false,
                TEXT,
            );
            c.text(
                tr("install.no_admin"),
                r.x + 28.,
                r.y + 164.,
                14,
                false,
                MUTED,
            );
        } else if let Some(operation) = setup {
            c.text(
                tr(match operation {
                    core_install_ui::SetupOperation::Initialize => "setup.confirm_initialize",
                    core_install_ui::SetupOperation::RegisterService => "setup.confirm_service",
                }),
                r.x + 28.,
                r.y + 80.,
                14,
                false,
                TEXT,
            );
            c.text(
                tr("setup.config_location"),
                r.x + 28.,
                r.y + 111.,
                12,
                true,
                MUTED,
            );
            c.ellipsis(
                self.state.get("status").get("configuration_path").text(),
                r.x + 28.,
                r.y + 131.,
                396.,
                14,
                false,
                TEXT,
            );
            c.text(
                tr("setup.no_start"),
                r.x + 28.,
                r.y + 164.,
                14,
                false,
                MUTED,
            );
        } else {
            c.text(
                tr("confirm.description"),
                r.x + 28.,
                r.y + 84.,
                15,
                false,
                MUTED,
            );
            c.text(
                tr("confirm.execute"),
                r.x + 28.,
                r.y + 112.,
                15,
                false,
                MUTED,
            );
        }
        if self.ui.button(
            c,
            9001,
            Rect::new(r.x + 192., r.y + height - 67., 104., 40.),
            tr("action.cancel"),
            Style::Quiet,
            true,
        ) {
            self.confirm = None;
        }
        if self.ui.button(
            c,
            9002,
            Rect::new(r.x + 312., r.y + height - 67., 112., 40.),
            tr("action.confirm"),
            Style::Primary,
            true,
        ) && let Some(action) = self.confirm.take()
        {
            self.provider_panel = 0;
            self.plugin_panel = 0;
            self.service_panel = false;
            self.protocol_menu = false;
            if let Some(operation) = action.install {
                self.core_operation(operation);
            } else if let Some(operation) = action.setup {
                self.setup_operation(operation);
            } else {
                self.action(&action.label, action.args, action.secret);
            }
        }
    }
    pub(super) fn settings_dialog(&mut self, c: &mut Canvas<'_>) {
        let r = sheet(c, tr("settings.title"));
        let x = r.x + 32.;
        let width = r.w - 64.;
        if self.ui.button(
            c,
            9400,
            Rect::new(r.x + r.w - 66., 25., 40., 40.),
            "×",
            Style::Quiet,
            true,
        ) {
            self.settings_panel = false;
            return;
        }
        c.text(tr("settings.title"), x, 100., 27, true, TEXT);
        c.ellipsis(tr("settings.help"), x, 148., width, 13, false, MUTED);
        c.text(tr("settings.theme"), x, 203., 14, true, TEXT);
        let themes = [
            settings::ThemeChoice::System,
            settings::ThemeChoice::Light,
            settings::ThemeChoice::Dark,
        ];
        let names = [
            tr("settings.system"),
            tr("settings.light"),
            tr("settings.dark"),
        ];
        let mut changed = false;
        for (index, theme) in themes.iter().enumerate() {
            let selected = self.preferences.theme == *theme;
            if self.ui.button(
                c,
                9410 + index as u64,
                Rect::new(x + index as f32 * 138., 234., 128., 42.),
                names[index],
                if selected {
                    Style::Primary
                } else {
                    Style::Secondary
                },
                true,
            ) && !selected
            {
                self.preferences.theme = *theme;
                changed = true;
            }
        }
        if self.ui.button(
            c,
            9420,
            Rect::new(x, 280., width, 33.),
            tr("settings.custom_colors"),
            Style::Secondary,
            true,
        ) {
            let theme = self.preferences.resolved_theme();
            self.failed = false;
            let preview = self.preferences.colors(theme);
            self.palette_editor = Some(ink::Editor {
                theme,
                preview,
                active: None,
                fields: preview.fields(),
            });
            self.ui = Ui::default();
            self.repaint = true;
        }
        c.text(tr("settings.language"), x, 316., 14, true, TEXT);
        for (index, language) in i18n::LANGUAGES.iter().enumerate() {
            let selected = self.preferences.language == *language;
            let label = if index == 0 {
                tr("settings.system")
            } else {
                i18n::LANGUAGE_NAMES[index - 1]
            };
            if self.ui.button(
                c,
                9430 + index as u64,
                Rect::new(x, 347. + index as f32 * 49., width, 40.),
                label,
                if selected {
                    Style::Primary
                } else {
                    Style::Secondary
                },
                true,
            ) && !selected
            {
                self.preferences.language = language;
                changed = true;
            }
        }
        if changed {
            match self.preferences.save() {
                Ok(()) => {
                    self.notice = tr("settings.saved").into();
                    self.failed = false;
                }
                Err(error) => {
                    self.notice = i18n::message("error.action", &[("error", &error)]);
                    self.failed = true;
                }
            }
            self.repaint = true;
        }
    }
    pub(super) fn palette_dialog(&mut self, c: &mut Canvas<'_>) {
        let r = sheet(c, tr("settings.custom_colors"));
        let x = r.x + 32.;
        let width = r.w - 64.;
        let editor = self
            .palette_editor
            .as_mut()
            .expect("palette editor is open");
        let label = if editor.theme == zgpui::native::Theme::Dark {
            "settings.dark"
        } else {
            "settings.light"
        };
        c.text(tr(label), x, 98., 25, true, TEXT);
        c.ellipsis(tr("settings.colors_help"), x, 134., width, 13, false, MUTED);
        let column = (width - 16.) / 2.;
        for (index, (label, field)) in ink::COLOR_LABELS.iter().zip(&mut editor.fields).enumerate()
        {
            let left = x + (index % 2) as f32 * (column + 16.);
            let top = 175. + (index / 2) as f32 * 62.;
            c.ellipsis(tr(label), left, top, column, 13, true, TEXT);
            let swatch = Rect::new(left, top + 21., 38., 34.);
            if self.ui.button(
                c,
                9530 + index as u64,
                swatch,
                "",
                if editor.active == Some(index) {
                    Style::Primary
                } else {
                    Style::Secondary
                },
                true,
            ) {
                editor.active = (editor.active != Some(index)).then_some(index);
                self.repaint = true;
            }
            if let Some(color) = ink::parse_color(field) {
                let _ = c.image(swatch.inset(6.), 1, 1, &color.to_be_bytes());
            }
            if self.ui.input(
                c,
                9500 + index as u64,
                Rect::new(left + 46., top + 21., column - 46., 34.),
                field,
                "#RRGGBB",
                false,
                true,
            ) {
                self.repaint = true;
                self.failed = false;
            }
        }

        if let Some(active) = editor.active {
            c.ellipsis(
                tr(ink::COLOR_LABELS[active]),
                x,
                427.,
                width,
                13,
                true,
                TEXT,
            );
            let swatch_width = width / ink::SWATCH_HUES.len() as f32;
            for (column, hue) in ink::SWATCH_HUES.into_iter().enumerate() {
                for shade in 0..5 {
                    let color = ink::swatch(hue, shade);
                    let position = Rect::new(
                        x + column as f32 * swatch_width,
                        449. + shade as f32 * 23.,
                        swatch_width - 3.,
                        22.,
                    );
                    let selected = ink::parse_color(&editor.fields[active]) == Some(color);
                    if self.ui.button(
                        c,
                        9540 + (shade * ink::SWATCH_HUES.len() + column) as u64,
                        position,
                        "",
                        if selected {
                            Style::Primary
                        } else {
                            Style::Secondary
                        },
                        true,
                    ) {
                        editor.fields[active] = format!("#{:06X}", color >> 8);
                        self.repaint = true;
                        self.failed = false;
                    }
                    let _ = c.image(position.inset(4.), 1, 1, &color.to_be_bytes());
                }
            }
        }

        let colors = ink::Colors::parse(&editor.fields);
        if let Some(colors) = colors {
            editor.preview = colors;
        }
        let bottom = if editor.active.is_some() {
            604.
        } else {
            (r.h - 69.).clamp(463., 500.)
        };
        if colors.is_none() || self.failed {
            let text = if colors.is_none() {
                tr("settings.colors_invalid")
            } else {
                &self.notice
            };
            c.ellipsis(text, x, bottom - 25., width, 13, false, RED);
        }
        if self.ui.button(
            c,
            9520,
            Rect::new(x, bottom, 132., 39.),
            tr("settings.colors_reset"),
            Style::Secondary,
            true,
        ) {
            editor.preview = ink::Colors::defaults(editor.theme);
            editor.fields = editor.preview.fields();
            self.failed = false;
            self.repaint = true;
        }
        if self.ui.button(
            c,
            9521,
            Rect::new(x + width - 216., bottom, 100., 39.),
            tr("action.cancel"),
            Style::Quiet,
            true,
        ) {
            self.palette_editor = None;
            self.ui = Ui::default();
            self.repaint = true;
        } else if self.ui.button(
            c,
            9522,
            Rect::new(x + width - 104., bottom, 104., 39.),
            tr("settings.colors_save"),
            Style::Primary,
            colors.is_some(),
        ) && let Some(colors) = colors
        {
            match self.preferences.save_colors(editor.theme, colors) {
                Ok(()) => {
                    self.palette_editor = None;
                    self.ui = Ui::default();
                    self.notice = tr("settings.saved").into();
                    self.failed = false;
                    self.repaint = true;
                }
                Err(error) => {
                    self.notice = i18n::message("error.action", &[("error", &error)]);
                    self.failed = true;
                    self.repaint = true;
                }
            }
        }
    }
}
