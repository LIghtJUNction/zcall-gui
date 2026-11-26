use crate::view_state::{CatalogState, GatewayState, OverviewAction};
use crate::*;
impl App {
    pub(super) fn overview(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let gateway = view_state::gateway_state(&self.state);
        let action = view_state::overview_action(&self.state);
        let healthy = gateway == GatewayState::Healthy;
        let missing = gateway == GatewayState::Unavailable && self.missing_gateway();
        let available = matches!(gateway, GatewayState::Healthy | GatewayState::Offline);
        let setup = self.setup_step();
        c.text(tr("nav.overview"), x, 34., 26, true, TEXT);
        c.ellipsis(tr("overview.subtitle"), x, 79., width, 14, false, MUTED);

        let top = 126.;
        let panel = Rect::new(x, top, width, 202.);
        c.rect(panel, 8., LINE);
        c.rect(panel.inset(1.), 7., SURFACE);
        brand(c, Rect::new(x + 22., top + 22., 40., 40.), SURFACE);
        let heading = match setup {
            Some(core_install_ui::SetupStep::Initialize) => tr("setup.config_needed"),
            Some(core_install_ui::SetupStep::RegisterService) if !healthy => {
                tr("setup.service_needed")
            }
            _ => match gateway {
                GatewayState::Loading => tr("overview.loading"),
                GatewayState::Unavailable => tr("overview.unavailable"),
                GatewayState::Offline => tr("overview.stopped"),
                GatewayState::Healthy => tr("overview.running"),
            },
        };
        c.ellipsis(
            heading,
            x + 77.,
            top + 22.,
            (width - 208.).max(150.),
            23,
            true,
            TEXT,
        );
        let status = match gateway {
            GatewayState::Loading => tr("status.loading_short"),
            GatewayState::Unavailable => tr("status.unknown"),
            GatewayState::Offline => tr("status.disconnected"),
            GatewayState::Healthy => tr("status.connected"),
        };
        let status_width = c.measure(status, 13, false);
        let status_x = x + width - status_width - 24.;
        c.rect(
            Rect::new(status_x - 12., top + 32., 5., 5.),
            2.5,
            if healthy { GREEN } else { MUTED },
        );
        c.text(
            status,
            status_x,
            top + 26.,
            13,
            false,
            if healthy { GREEN } else { MUTED },
        );
        c.ellipsis(
            view_state::endpoint(&self.state).unwrap_or(tr("overview.endpoint_unknown")),
            x + 77.,
            top + 60.,
            width - 101.,
            15,
            false,
            MUTED,
        );
        let detail = self.setup_help().unwrap_or_else(|| match action {
            OverviewAction::Wait => tr("status.loading_help"),
            OverviewAction::Retry if healthy => tr("overview.providers_unavailable"),
            OverviewAction::Retry => tr("overview.unavailable_help"),
            OverviewAction::StartGateway => tr("overview.start_help"),
            OverviewAction::AddProvider => tr("overview.add_help"),
            OverviewAction::ConnectTools => tr("overview.ready_help"),
        });
        c.ellipsis(detail, x + 24., top + 102., width - 48., 14, false, MUTED);
        let label = if missing {
            tr("install.bootstrap")
        } else if let Some(step) = setup.filter(|_| !healthy) {
            tr(match step {
                core_install_ui::SetupStep::Initialize => "setup.initialize",
                core_install_ui::SetupStep::RegisterService => "setup.install_service",
                core_install_ui::SetupStep::Ready => "action.start_gateway",
                core_install_ui::SetupStep::Inspect => "action.retry",
            })
        } else {
            match action {
                OverviewAction::Wait => tr("status.loading_short"),
                OverviewAction::Retry => tr("action.retry"),
                OverviewAction::StartGateway => tr("action.start_gateway"),
                OverviewAction::AddProvider => tr("action.add_provider"),
                OverviewAction::ConnectTools => tr("action.connect_tools"),
            }
        };
        let button_width = (c.measure(label, 14, false) + 38.).clamp(162., 270.);
        if self.ui.button(
            c,
            100,
            Rect::new(x + 24., top + 145., button_width, 38.),
            label,
            Style::Primary,
            self.enabled() && action != OverviewAction::Wait,
        ) {
            if missing {
                self.start_core_install();
            } else if let Some(step) = setup.filter(|_| !healthy) {
                self.trigger_setup_step(step);
            } else {
                match action {
                    OverviewAction::ConnectTools => {
                        self.page = 2;
                        self.scroll = 0;
                        self.repaint = true;
                    }
                    OverviewAction::AddProvider => {
                        self.page = 1;
                        self.scroll = 0;
                        self.repaint = true;
                        self.clear_key();
                        self.provider = ProviderForm::default();
                        self.provider_edit = false;
                        self.provider_panel = 2;
                    }
                    OverviewAction::StartGateway => self.ask(
                        tr("action.start_service_confirm"),
                        words(&["service", "start"]),
                    ),
                    OverviewAction::Retry => self.refresh(),
                    OverviewAction::Wait => {}
                }
            }
        }
        if available
            && self.ui.button(
                c,
                101,
                Rect::new(x + button_width + 40., top + 145., 156., 38.),
                tr("action.service_settings"),
                Style::Quiet,
                self.enabled(),
            )
        {
            self.service_panel = true;
        }

        let providers = self.state.get("providers").get("providers");
        let agents = self.state.get("agents").get("agents");
        let provider_count = if view_state::collection_state(&self.state, "providers", "providers")
            == LoadState::Ready
        {
            providers.array().len().to_string()
        } else {
            "—".to_owned()
        };
        let agent_count =
            if view_state::collection_state(&self.state, "agents", "agents") == LoadState::Ready {
                agents
                    .array()
                    .iter()
                    .filter(|agent| agent.get("installed").boolean())
                    .count()
                    .to_string()
            } else {
                "—".to_owned()
            };
        let tile_width = (width - 16.) / 2.;
        for (index, (label, count, page)) in [
            (tr("overview.providers_count"), provider_count, 1),
            (tr("overview.agents_count"), agent_count, 2),
        ]
        .iter()
        .enumerate()
        {
            let left = x + index as f32 * (tile_width + 16.);
            let tile = Rect::new(left, top + 222., tile_width, 120.);
            c.rect(tile, 8., SURFACE);
            if self.ui.button(
                c,
                110 + index as u64,
                tile,
                "",
                Style::Quiet,
                self.enabled(),
            ) {
                self.page = *page;
                self.scroll = 0;
                self.repaint = true;
            }
            c.text(count, left + 22., top + 242., 30, true, TEXT);
            c.ellipsis(
                label,
                left + 22.,
                top + 290.,
                tile_width - 44.,
                13,
                false,
                MUTED,
            );
            c.text("›", left + tile_width - 28., top + 262., 24, false, MUTED);
        }
        c.ellipsis(
            tr("overview.independent"),
            x,
            top + 374.,
            width,
            13,
            false,
            MUTED,
        );
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
        if self.ui.input(
            c,
            401,
            Rect::new(x, 118., width, 42.),
            &mut self.provider_query,
            tr("providers.search"),
            false,
            enabled,
        ) {
            self.scroll = 0;
        }
        let query = self.provider_query.trim().to_lowercase();
        let providers = self.state.get("providers").get("providers").array();
        if providers.is_empty() {
            empty(
                c,
                Rect::new(x, 190., width, 172.),
                tr("providers.empty"),
                tr("providers.empty_help"),
            );
            return;
        }
        // Rendering borrows the snapshot. Copy only the one selected provider.
        let items = providers
            .iter()
            .filter(|item| {
                query.is_empty()
                    || ["name", "base_url", "model", "protocol"]
                        .iter()
                        .any(|key| item.get(key).text().to_lowercase().contains(&query))
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            empty(
                c,
                Rect::new(x, 190., width, 172.),
                tr("providers.no_matches"),
                tr("providers.no_matches_help"),
            );
            return;
        }
        c.text(
            &i18n::message("providers.count", &[("count", &items.len().to_string())]),
            x,
            179.,
            13,
            false,
            MUTED,
        );
        let rows = ((c.height - 285.) / 80.).max(1.) as usize;
        self.scroll = self.scroll.min(items.len().saturating_sub(rows));
        let visible = items.len().saturating_sub(self.scroll).min(rows);
        c.rect(Rect::new(x, 211., width, visible as f32 * 80.), 8., SURFACE);
        let mut selected_provider = None;
        for (index, item) in items.iter().enumerate().skip(self.scroll).take(rows) {
            let y = 211. + (index - self.scroll) as f32 * 80.;
            let r = Rect::new(x + 1., y + 1., width - 2., 78.);
            if self
                .ui
                .button(c, 10000 + index as u64, r, "", Style::Quiet, enabled)
            {
                selected_provider = Some((**item).clone());
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
        if let Some(item) = selected_provider {
            self.clear_key();
            self.provider = ProviderForm {
                name: value(&item, "name", "provider").into(),
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
    }

    pub(super) fn agents(&mut self, c: &mut Canvas<'_>, x: f32, width: f32) {
        let enabled = self.enabled();
        let catalog = view_state::catalog_state(&self.state);
        let has_routes = catalog == CatalogState::Available;
        if self.ui.button(
            c,
            501,
            Rect::new(x + width - 166., 35., 166., 42.),
            tr("action.detect_agents"),
            Style::Secondary,
            enabled,
        ) {
            self.action("Sync AI tool status", words(&["agents", "detect"]), None);
        }
        if has_routes {
            c.ellipsis(
                tr("agents.selection_help"),
                x,
                132.,
                width,
                14,
                false,
                MUTED,
            );
        } else {
            let (heading, help) = match catalog {
                CatalogState::Loading => ("agents.routes_loading", "status.loading_help"),
                CatalogState::Unavailable => {
                    ("agents.routes_unavailable", "agents.routes_retry_help")
                }
                CatalogState::Empty => ("agents.no_routes", "agents.no_routes_help"),
                CatalogState::Available => unreachable!(),
            };
            c.ellipsis(tr(heading), x, 125., width - 186., 14, true, TEXT);
            c.ellipsis(tr(help), x, 151., width - 186., 13, false, MUTED);
            if catalog != CatalogState::Loading
                && self.ui.button(
                    c,
                    502,
                    Rect::new(x + width - 166., 122., 166., 42.),
                    tr(if catalog == CatalogState::Unavailable {
                        "action.retry"
                    } else {
                        "action.add_provider"
                    }),
                    if catalog == CatalogState::Unavailable {
                        Style::Secondary
                    } else {
                        Style::Primary
                    },
                    enabled,
                )
            {
                if catalog == CatalogState::Unavailable {
                    self.refresh();
                } else {
                    self.page = 1;
                    self.scroll = 0;
                    self.repaint = true;
                    self.clear_key();
                    self.provider = ProviderForm::default();
                    self.provider_edit = false;
                    self.provider_panel = 2;
                }
                return;
            }
        }
        let items = self.state.get("agents").get("agents").array().to_vec();
        let compact = width < 620.;
        let stride = if compact { 152. } else { 104. };
        let rows = ((c.height - 289.) / stride).max(1.) as usize;
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
            let y = 189. + (index - self.scroll) as f32 * stride;
            let name = value(item, "name", "tool");
            let configured = item.get("configured").boolean();
            let installed = item.get("installed").boolean();
            let writable = item.get("writable").boolean();
            let changed = item.get("changed").boolean();
            let restorable = item.get("restorable").boolean();
            c.rect(Rect::new(x, y, width, stride - 12.), 10., SURFACE);
            agent_identity::draw(c, name, Rect::new(x + 20., y + 18., 40., 40.));
            let label_x = x + 80.;
            let actions = writable && (installed || configured);
            let reserved = if compact || (!actions && !restorable) {
                22.
            } else if restorable {
                264.
            } else {
                170.
            };
            let label_width = (x + width - reserved - label_x).max(0.);
            c.ellipsis(
                agent_identity::display_name(name),
                label_x,
                y + 13.,
                label_width,
                18,
                true,
                TEXT,
            );
            let status_color = if configured && writable && !changed {
                GREEN
            } else {
                MUTED
            };
            c.rect(Rect::new(label_x, y + 45., 5., 5.), 2.5, status_color);
            c.ellipsis(
                if changed && configured {
                    tr("agents.changed_connected")
                } else if changed {
                    tr("agents.changed_disconnected")
                } else if !installed && !configured {
                    tr("agents.not_installed")
                } else if !writable {
                    tr("agents.read_only")
                } else if configured {
                    tr("agents.connected")
                } else {
                    tr("agents.available")
                },
                label_x + 13.,
                y + 39.,
                (label_width - 13.).max(0.),
                13,
                false,
                status_color,
            );
            if configured || !item.get("route").text().is_empty() {
                c.ellipsis(
                    &i18n::message(
                        if configured {
                            "agents.current_route"
                        } else {
                            "agents.saved_route"
                        },
                        &[("route", value(item, "route", tr("agents.route_unset")))],
                    ),
                    label_x,
                    y + 64.,
                    (width - 102.).max(0.),
                    12,
                    false,
                    MUTED,
                );
            }
            let action_y = y + if compact { 96. } else { 20. };
            if actions
                && self.ui.button(
                    c,
                    20000 + index as u64 * 2,
                    Rect::new(x + width - 154., action_y, 132., 38.),
                    tr(if configured {
                        "action.change_route"
                    } else {
                        "action.connect"
                    }),
                    Style::Secondary,
                    enabled && installed && writable && has_routes,
                )
            {
                self.open_route_picker(item);
                return;
            }
            if restorable
                && self.ui.button(
                    c,
                    20001 + index as u64 * 2,
                    Rect::new(x + width - 248., action_y, 80., 38.),
                    tr("action.restore"),
                    Style::Quiet,
                    enabled && writable,
                )
            {
                self.ask(
                    &i18n::message("action.restore_named", &[("name", name)]),
                    words(&["agents", "revert", name]),
                );
            }
        }
        if !items.is_empty() {
            c.ellipsis(
                &i18n::message(
                    "agents.visible_count",
                    &[
                        ("start", &(self.scroll + 1).to_string()),
                        ("end", &(self.scroll + rows).min(items.len()).to_string()),
                        ("total", &items.len().to_string()),
                    ],
                ),
                x,
                c.height - 73.,
                (width - 150.).max(0.),
                12,
                false,
                MUTED,
            );
            if self.ui.button(
                c,
                503,
                Rect::new(x + width - 126., c.height - 82., 54., 30.),
                "‹",
                Style::Quiet,
                enabled && self.scroll > 0,
            ) {
                self.scroll = self.scroll.saturating_sub(rows);
                self.repaint = true;
            }
            if self.ui.button(
                c,
                504,
                Rect::new(x + width - 62., c.height - 82., 54., 30.),
                "›",
                Style::Quiet,
                enabled && self.scroll + rows < items.len(),
            ) {
                self.scroll += rows;
                self.repaint = true;
            }
        }
    }

    pub(super) fn agent_route_dialog(&mut self, c: &mut Canvas<'_>) {
        let routes = self.route_choices();
        let enabled = !self.busy && self.confirm.is_none();
        let Some(picker) = self.route_picker.as_mut() else {
            return;
        };
        let width = 580_f32.min(c.width - 48.);
        let height = 600_f32.min(c.height - 48.);
        let r = Rect::new(
            ((c.width - width) / 2.).round(),
            ((c.height - height) / 2.).round(),
            width,
            height,
        );
        c.rect(Rect::new(0., 0., c.width, c.height), 0., 0x00000066);
        c.rect(r, 10., SURFACE);
        let x = r.x + 24.;
        let inner = r.w - 48.;
        c.ellipsis(
            &i18n::message(
                "agents.choose_for",
                &[("name", agent_identity::display_name(&picker.agent))],
            ),
            x + 40.,
            r.y + 24.,
            inner - 88.,
            22,
            true,
            TEXT,
        );
        agent_identity::draw(c, &picker.agent, Rect::new(x, r.y + 23., 28., 28.));
        c.ellipsis(
            &i18n::message(
                "agents.current_route",
                &[(
                    "route",
                    if picker.current.is_empty() {
                        tr("agents.route_unset")
                    } else {
                        &picker.current
                    },
                )],
            ),
            x,
            r.y + 63.,
            inner,
            13,
            false,
            MUTED,
        );
        if self.ui.input(
            c,
            25000,
            Rect::new(x, r.y + 96., inner, 42.),
            &mut picker.query,
            tr("agents.route_search"),
            false,
            enabled,
        ) {
            picker.scroll = 0;
        }
        let filtered = routes
            .iter()
            .enumerate()
            .filter(|(_, route)| route.matches(&picker.query))
            .collect::<Vec<_>>();
        let visible = ((r.h - 294.) / 54.).max(1.) as usize;
        picker.scroll = picker.scroll.min(filtered.len().saturating_sub(visible));
        if filtered.is_empty() {
            c.ellipsis(
                tr("agents.no_matches"),
                x,
                r.y + 175.,
                inner,
                16,
                true,
                TEXT,
            );
            c.ellipsis(
                tr("agents.no_matches_help"),
                x,
                r.y + 204.,
                inner,
                13,
                false,
                MUTED,
            );
        }
        for (position, (index, route)) in filtered
            .iter()
            .skip(picker.scroll)
            .take(visible)
            .enumerate()
        {
            let row = Rect::new(x, r.y + 154. + position as f32 * 54., inner, 48.);
            let selected = picker.selected == route.id;
            if selected {
                c.rect(row, 6., BLUE_PALE);
            }
            if self
                .ui
                .button(c, 50000 + *index as u64, row, "", Style::Quiet, enabled)
            {
                picker.selected = route.id.clone();
            }
            if selected {
                c.rect(Rect::new(row.x + 3., row.y + 12., 3., 24.), 1.5, BLUE);
            }
            c.ellipsis(
                &route.label,
                row.x + 16.,
                row.y + 5.,
                row.w - 32.,
                15,
                selected,
                TEXT,
            );
            c.ellipsis(
                &route.id,
                row.x + 16.,
                row.y + 28.,
                row.w - 32.,
                11,
                false,
                MUTED,
            );
        }
        c.ellipsis(
            &i18n::message(
                "agents.route_count",
                &[("count", &filtered.len().to_string())],
            ),
            x,
            r.y + r.h - 125.,
            inner - 160.,
            12,
            false,
            MUTED,
        );
        if self.ui.button(
            c,
            25002,
            Rect::new(r.x + r.w - 162., r.y + r.h - 132., 58., 30.),
            "‹",
            Style::Quiet,
            enabled && picker.scroll > 0,
        ) {
            picker.scroll = picker.scroll.saturating_sub(visible);
        }
        if self.ui.button(
            c,
            25003,
            Rect::new(r.x + r.w - 90., r.y + r.h - 132., 58., 30.),
            "›",
            Style::Quiet,
            enabled && picker.scroll + visible < filtered.len(),
        ) {
            picker.scroll += visible;
        }
        c.rect(Rect::new(x, r.y + r.h - 94., inner, 1.), 0., LINE);
        c.ellipsis(
            if picker.selected.is_empty() {
                tr("agents.select_route")
            } else {
                &picker.selected
            },
            x,
            r.y + r.h - 66.,
            inner - 164.,
            13,
            false,
            MUTED,
        );
        let valid = routes.iter().any(|route| route.id == picker.selected);
        if self.ui.button(
            c,
            25004,
            Rect::new(r.x + r.w - 170., r.y + r.h - 72., 146., 42.),
            tr("agents.apply_route"),
            Style::Primary,
            enabled && valid,
        ) {
            let agent = picker.agent.clone();
            let route = picker.selected.clone();
            self.route_picker = None;
            self.ui = Ui::default();
            self.ask(
                &i18n::message("agents.apply_named", &[("name", &agent), ("route", &route)]),
                words(&["agents", "apply", &agent, "--route", &route]),
            );
            return;
        }
        // Register the close control last so Tab starts in search and then results.
        if self.ui.button(
            c,
            25001,
            Rect::new(r.x + r.w - 62., r.y + 14., 40., 40.),
            "×",
            Style::Quiet,
            enabled,
        ) {
            self.route_picker = None;
            self.ui = Ui::default();
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
            c.rect(r, 8., if selected { BLUE_PALE } else { LINE });
            c.rect(r.inset(1.), 7., if selected { BLUE_PALE } else { SURFACE });
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
        let gateway_version = match view_state::gateway_state(&self.state) {
            GatewayState::Loading => tr("status.loading_short"),
            GatewayState::Unavailable => tr("status.unknown"),
            GatewayState::Healthy | GatewayState::Offline => {
                value(self.state.get("status"), "version", tr("status.unknown"))
            }
        };
        let items = [
            (
                tr("updates.gateway"),
                gateway_version.to_owned(),
                self.gateway_update.clone(),
            ),
            (
                tr("updates.gui"),
                VERSION.to_owned(),
                self.gui_update.clone(),
            ),
        ];
        for (index, (title, current, info)) in items.iter().enumerate() {
            let y = if index == 0 { 140. } else { 340. };
            if index == 0 {
                self.gateway_install_card(c, x, y, width, title, current, info, enabled);
                continue;
            }
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
                } else {
                    self.self_update(update);
                }
            }
        }
        let note_y = (c.height - 100.).min(511.);
        c.ellipsis(
            tr("updates.verify"),
            x + 2.,
            note_y,
            width - 4.,
            14,
            false,
            MUTED,
        );
        c.ellipsis(
            tr("updates.managed_help"),
            x + 2.,
            note_y + 24.,
            width - 4.,
            14,
            false,
            MUTED,
        );
    }
}
