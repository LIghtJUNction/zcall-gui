use crate::*;
fn sheet(c: &mut Canvas<'_>, heading: &str) -> Rect {
    c.rect(Rect::new(0., 0., c.width, c.height), 0., 0x20212438);
    let r = Rect::new((c.width - 468.).max(0.), 0., 468., c.height);
    c.rect(r, 0., SURFACE);
    c.rect(Rect::new(r.x, 0., 1., r.h), 0., LINE);
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
            } else if self.provider_edit {
                tr("providers.edit")
            } else {
                tr("action.add_provider")
            },
        );
        let x = r.x + 32.;
        let width = r.w - 64.;
        let enabled = !self.busy && self.confirm.is_none();
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
            c.rect(Rect::new(x, r.h - 204., width, 1.), 0., LINE);
            c.text(tr("providers.remove_title"), x, r.h - 181., 14, true, TEXT);
            c.text(tr("providers.remove_help"), x, r.h - 152., 13, false, MUTED);
            if self.ui.button(
                c,
                9103,
                Rect::new(x, r.h - 101., 140., 40.),
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
                c.ellipsis(&self.notice, x, 412., width, 13, false, RED);
            } else if self.busy {
                c.text(tr("status.testing"), x, 412., 14, false, MUTED);
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
                field(
                    &mut self.ui,
                    c,
                    9121,
                    x,
                    296.,
                    width,
                    tr("providers.url"),
                    &mut self.provider.url,
                    "https://api.example.com/v1",
                    false,
                    enabled && !self.protocol_menu,
                );
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
            let valid =
                !self.provider.name.trim().is_empty() && !self.provider.url.trim().is_empty();
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
                    self.provider_panel = 3;
                } else {
                    let mut args = words(&[
                        "providers",
                        "add",
                        self.provider.name.trim(),
                        "--base-url",
                        self.provider.url.trim(),
                        "--protocol",
                        protocols()[self.provider.protocol],
                    ]);
                    if !self.provider.model.trim().is_empty() {
                        args.extend(words(&["--model", self.provider.model.trim()]));
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
            if self.protocol_menu {
                let y = (r.h - 332.).min(474.);
                card(c, Rect::new(x, y, width, 216.));
                for i in 0..6 {
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
                            BLUE
                        } else {
                            TEXT
                        },
                    );
                }
            }
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
                if on { GREEN } else { MUTED },
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
        let healthy = self.state.get("status").get("healthy").boolean();
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
        c.text(
            if healthy {
                tr("service.connected")
            } else {
                tr("service.disconnected")
            },
            x,
            100.,
            27,
            true,
            TEXT,
        );
        c.text(tr("service.independent"), x, 154., 14, false, MUTED);
        if self.ui.button(
            c,
            9301,
            Rect::new(x, 225., 142., 44.),
            if healthy {
                tr("action.restart_service")
            } else {
                tr("action.start_service")
            },
            Style::Primary,
            enabled,
        ) {
            self.ask(
                if healthy {
                    tr("action.restart_service_confirm")
                } else {
                    tr("action.start_service_confirm")
                },
                words(&["service", if healthy { "restart" } else { "start" }]),
            );
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
        c.rect(Rect::new(0., 0., c.width, c.height), 0., 0x20212455);
        let r = Rect::new(
            ((c.width - 452.) / 2.).round(),
            ((c.height - 230.) / 2.).round(),
            452.,
            230.,
        );
        c.rect(r, 20., SURFACE);
        let label = self.confirm.as_ref().unwrap().label.clone();
        c.ellipsis(&label, r.x + 28., r.y + 30., 396., 23, true, TEXT);
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
        if self.ui.button(
            c,
            9001,
            Rect::new(r.x + 192., r.y + 163., 104., 40.),
            tr("action.cancel"),
            Style::Quiet,
            true,
        ) {
            self.confirm = None;
        }
        if self.ui.button(
            c,
            9002,
            Rect::new(r.x + 312., r.y + 163., 112., 40.),
            tr("action.confirm"),
            Style::Primary,
            true,
        ) && let Some(action) = self.confirm.take()
        {
            self.provider_panel = 0;
            self.plugin_panel = 0;
            self.service_panel = false;
            self.protocol_menu = false;
            self.action(&action.label, action.args, action.secret);
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
}
