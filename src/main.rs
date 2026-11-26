#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use ink::{ACCENT, ACCENT_SOFT, SUCCESS};
mod agent_identity;
mod agent_logos;
mod backend;
mod core_install;
mod core_install_io;
mod core_install_ui;
mod dialogs;
mod i18n;
mod ink;
mod json;
mod package_update;
mod pages;
mod provider_models;
mod provider_oauth;
mod settings;
mod updater;
mod usage;
mod view_state;
use backend::{Backend, Reply};
use i18n::tr;
use json::Json;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use view_state::LoadState;
use zgpui::native::{ButtonStyle as Style, Canvas, Event, Rect, Ui, Window, colors::*};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE_PAGE: usize = 5;
const UPDATES_PAGE: usize = 6;
const PAGES: [&str; 7] = [
    "nav.overview",
    "nav.providers",
    "nav.agents",
    "nav.plugins",
    "nav.privacy",
    "nav.usage",
    "nav.updates",
];
const PAGE_IDS: [&str; 7] = [
    "overview",
    "providers",
    "agents",
    "plugins",
    "privacy",
    "usage",
    "updates",
];
#[derive(Default)]
struct ProviderForm {
    name: String,
    url: String,
    model: String,
    environment: String,
    route: String,
    authentication: usize,
    header_refs: String,
    original_authentication: usize,
    original_header_refs: String,
    key: String,
    protocol: usize,
    oauth: bool,
    plugin_id: String,
    oauth_method: Option<usize>,
    plugin_endpoint: String,
    pending_oauth_binding: Json,
    plugin_picker: bool,
    plugin_catalog: Json,
    plugin_market: Json,
    browse_market: bool,
    plugin_query: String,
    plugin_offset: usize,
    endpoint_menu: bool,
    endpoint_filter: bool,
}
#[derive(Default)]
struct PluginForm {
    source: String,
    revision: String,
    name: String,
    sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct RouteChoice {
    id: String,
    label: String,
}
impl RouteChoice {
    fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        self.id.to_lowercase().contains(&query) || self.label.to_lowercase().contains(&query)
    }
}
struct AgentRoutePicker {
    agent: String,
    current: String,
    selected: String,
    query: String,
    scroll: usize,
}
struct Action {
    label: String,
    args: Vec<String>,
    secret: Option<Vec<u8>>,
    install: Option<core_install_ui::Operation>,
    setup: Option<core_install_ui::SetupOperation>,
}
struct App {
    backend: Backend,
    ui: Ui,
    page: usize,
    scroll: usize,
    state: Json,
    usage: Json,
    usage_restart_pending: bool,
    busy: bool,
    notice: String,
    failed: bool,
    sender: mpsc::Sender<Reply>,
    receiver: mpsc::Receiver<Reply>,
    provider: ProviderForm,
    oauth_input: Option<mpsc::Sender<String>>,
    oauth_code: String,
    oauth_instructions: String,
    oauth_expects_code: bool,
    oauth_prompt: Json,
    oauth_prompt_offset: usize,
    provider_query: String,
    provider_models: Option<provider_models::Browser>,
    plugin: PluginForm,
    route_picker: Option<AgentRoutePicker>,
    confirm: Option<Action>,
    gateway_update: Json,
    gui_update: Json,
    core_installation: Json,
    install_details: bool,
    setup_service_verified: Option<core_install_ui::SetupContext>,
    quit: bool,
    provider_panel: u8,
    provider_edit: bool,
    plugin_panel: u8,
    selected_plugin: Json,
    service_panel: bool,
    protocol_menu: bool,
    preferences: settings::Preferences,
    settings_panel: bool,
    palette_editor: Option<ink::Editor>,
    repaint: bool,
    executable: Option<std::path::PathBuf>,
    privacy_restart_pending: bool,
}
fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).into()).collect()
}
fn value<'a>(j: &'a Json, key: &str, fallback: &'a str) -> &'a str {
    let s = j.get(key).text();
    if s.is_empty() { fallback } else { s }
}
fn human(s: &str) -> &str {
    match s {
        "Magpie provider market" | "Resolve Magpie plugin" => tr("providers.oauth_magpie_market"),
        "Discover OAuth plugins" => tr("providers.oauth_choose_plugin"),
        "OAuth provider" => tr("providers.oauth_login"),
        "Save provider" => tr("action.save_provider"),
        "Fetch provider models" => tr("providers.browse_models"),
        "Test provider" => tr("action.test_provider"),
        "Sync AI tool status" => tr("action.detect_agents"),
        "Set privacy mode" => tr("action.privacy"),
        "Check gateway update" => tr("action.check_gateway"),
        "Check GUI update" => tr("action.check_gui"),
        "Update GUI" => tr("action.update_gui"),
        "Start gateway" => tr("action.start_gateway"),
        "Restart gateway" => tr("action.restart_gateway"),
        "Stop gateway" => tr("action.stop_gateway"),
        "Bootstrap initialize" => tr("setup.initialize"),
        "Bootstrap service install" => tr("setup.install_service"),
        _ => s,
    }
}
impl App {
    fn new(backend: Backend, preferences: settings::Preferences) -> Self {
        let app = Self::unloaded(backend, preferences);
        app.spawn_refresh();
        app
    }
    fn unloaded(backend: Backend, preferences: settings::Preferences) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            backend,
            ui: Ui::default(),
            page: 0,
            scroll: 0,
            state: Json::Null,
            usage: Json::Null,
            usage_restart_pending: false,
            busy: true,
            notice: tr("status.loading").into(),
            failed: false,
            sender,
            receiver,
            provider: ProviderForm::default(),
            oauth_input: None,
            oauth_code: String::new(),
            oauth_instructions: String::new(),
            oauth_expects_code: false,
            oauth_prompt: Json::Null,
            oauth_prompt_offset: 0,
            provider_query: String::new(),
            provider_models: None,
            plugin: PluginForm::default(),
            route_picker: None,
            confirm: None,
            gateway_update: Json::Null,
            gui_update: Json::Null,
            core_installation: Json::Null,
            install_details: false,
            setup_service_verified: None,
            quit: false,
            provider_panel: 0,
            provider_edit: false,
            plugin_panel: 0,
            selected_plugin: Json::Null,
            service_panel: false,
            protocol_menu: false,
            preferences,
            settings_panel: false,
            palette_editor: None,
            repaint: false,
            executable: std::env::current_exe().ok(),
            privacy_restart_pending: false,
        }
    }
    fn modal(&self) -> bool {
        self.confirm.is_some()
            || self.provider_models.is_some()
            || self.provider_panel > 0
            || self.plugin_panel > 0
            || self.service_panel
            || self.settings_panel
            || self.route_picker.is_some()
            || self.install_details
    }
    fn enabled(&self) -> bool {
        !self.busy && !self.modal()
    }
    fn route_choices(&self) -> Vec<RouteChoice> {
        // The controller's models command uses the real OpenAI catalog shape.
        let mut routes = Vec::new();
        for item in self.state.get("models").get("data").array() {
            let Json::String(id) = item.get("id") else {
                continue;
            };
            let id = id.as_str();
            if id.trim().is_empty() || routes.iter().any(|route: &RouteChoice| route.id == id) {
                continue;
            }
            routes.push(RouteChoice {
                id: id.into(),
                label: value(item, "name", id).into(),
            });
        }
        routes.sort_by(|a, b| {
            a.label
                .to_lowercase()
                .cmp(&b.label.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        routes
    }
    fn open_route_picker(&mut self, agent: &Json) {
        let routes = self.route_choices();
        let name = agent.get("name").text();
        if self.busy
            || name.is_empty()
            || !agent.get("installed").boolean()
            || !agent.get("writable").boolean()
            || routes.is_empty()
        {
            return;
        }
        let current = agent.get("route").text().to_owned();
        let selected = routes
            .iter()
            .find(|route| route.id == current)
            .map(|route| route.id.clone())
            .unwrap_or_default();
        self.route_picker = Some(AgentRoutePicker {
            agent: name.into(),
            current,
            selected,
            query: String::new(),
            scroll: 0,
        });
        // The first Tab enters the picker, never a background-page control.
        self.ui = Ui::default();
    }
    fn refresh(&mut self) {
        if self.page == USAGE_PAGE {
            self.refresh_usage();
            return;
        }
        if !self.busy {
            self.busy = true;
            self.notice = tr("status.loading").into();
            self.failed = false;
            self.spawn_refresh();
        }
    }
    fn action(&mut self, label: &str, args: Vec<String>, secret: Option<Vec<u8>>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.failed = false;
        self.notice = i18n::message("status.working", &[("action", human(label))]);
        self.backend
            .spawn(self.sender.clone(), label.into(), args, secret);
    }
    fn ask(&mut self, label: &str, args: Vec<String>) {
        self.confirm = Some(Action {
            label: label.into(),
            args,
            secret: None,
            install: None,
            setup: None,
        });
    }
    fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(reply) = self.receiver.try_recv() {
            changed = true;
            self.busy = false;
            if reply.label == usage::READ_LABEL {
                self.accept_usage(reply.result);
                continue;
            }
            if matches!(
                reply.label.as_str(),
                usage::ENABLE_LABEL | usage::DISABLE_LABEL
            ) {
                self.accept_usage_recording(reply.result);
                continue;
            }
            if matches!(
                reply.label.as_str(),
                "Discover OAuth plugins" | "Magpie provider market" | "Resolve Magpie plugin"
            ) {
                match reply.result {
                    Ok(data) if reply.label == "Resolve Magpie plugin" => {
                        let source = data.get("source").text();
                        let name = data.get("name").text();
                        if (source.starts_with("npm:") || source.starts_with("magpie:"))
                            && !name.is_empty()
                        {
                            let spec = source.split('#').next().unwrap_or(source);
                            let title = format!("{} · {}", tr("action.install_plugin"), spec);
                            self.ask(
                                &title,
                                words(&["plugins", "install", source, "--name", name]),
                            );
                        }
                    }
                    Ok(data) if reply.label == "Magpie provider market" => {
                        self.provider.plugin_market = data;
                        self.notice = tr("status.refreshed").into();
                    }
                    Ok(data) => {
                        self.provider.plugin_catalog = data;
                        self.notice = tr("status.refreshed").into();
                    }
                    Err(error) => {
                        self.notice = i18n::message("error.action", &[("error", &error)]);
                        self.failed = true;
                    }
                }
                continue;
            }
            if reply.label == "OAuth prompt" {
                self.busy = true;
                self.oauth_code.clear();
                self.oauth_prompt_offset = 0;
                self.oauth_prompt = reply.result.unwrap_or_default();
                continue;
            }
            if matches!(reply.label.as_str(), "OAuth code" | "OAuth browser") {
                self.busy = true;
                self.oauth_expects_code = reply.label == "OAuth code";
                self.oauth_prompt = Json::Null;
                self.oauth_instructions = reply.result.map_or_else(
                    |_| tr("providers.oauth_code_help").to_owned(),
                    |data| {
                        let text = data.text();
                        if text.is_empty() {
                            tr(if self.oauth_expects_code {
                                "providers.oauth_code_help"
                            } else {
                                "providers.oauth_wait"
                            })
                            .into()
                        } else {
                            text.into()
                        }
                    },
                );
                continue;
            }
            if reply.label == "OAuth provider" {
                self.oauth_input = None;
                self.oauth_prompt = Json::Null;
                self.oauth_code.clear();
                self.oauth_instructions.clear();
            }
            if reply.label == "Fetch provider models" {
                if let Some(browser) = self.provider_models.as_mut() {
                    self.failed = !browser.accept(reply.result);
                    self.notice = tr(if self.failed {
                        "providers.models_failed"
                    } else {
                        "providers.models_loaded"
                    })
                    .into();
                }
                continue;
            }
            match reply.result {
                Ok(data)
                    if reply.label == "OAuth provider" && data.get("needs_endpoint").boolean() =>
                {
                    self.provider.pending_oauth_binding = data;
                    self.notice = tr("providers.oauth_no_endpoint").into();
                    self.failed = true;
                }
                Ok(data) if reply.label == "refresh" => {
                    if data.get("installation") != &Json::Null {
                        self.accept_installation(data.get("installation").clone(), false, false);
                    }
                    self.state = data;
                    if self.setup_step() == Some(core_install_ui::SetupStep::Initialize) {
                        self.notice = tr("setup.initialize_help").into();
                        self.failed = false;
                    } else if view_state::gateway_state(&self.state)
                        == view_state::GatewayState::Unavailable
                    {
                        let error = value(
                            self.state.get("status"),
                            "error",
                            tr("error.invalid_response"),
                        );
                        self.notice = i18n::message("error.connection", &[("error", error)]);
                        self.failed = true;
                    } else {
                        self.notice = tr("status.refreshed").into();
                        self.failed = false;
                    }
                }
                Ok(data) if reply.label == "Sync AI tool status" => {
                    if let (Json::Object(state), Json::Array(_)) =
                        (&mut self.state, data.get("agents"))
                    {
                        state.insert("agents".into(), data);
                        self.notice = tr("agents.synced").into();
                        self.failed = false;
                    } else {
                        self.notice = tr("error.invalid_response").into();
                        self.failed = true;
                    }
                }
                Ok(data) if reply.label.starts_with("Core installer ") => {
                    let prepare = reply.label == "Core installer prepare";
                    let completed = core_install_ui::committed(&data);
                    self.accept_installation(data, prepare, true);
                    if completed && !self.failed {
                        self.spawn_refresh();
                        self.busy = true;
                    }
                }
                Ok(data) => {
                    self.accept_setup(&reply.label, &data);
                    if reply.starts_gateway {
                        self.privacy_restart_pending = false;
                        self.usage_restart_pending = false;
                    }
                    if reply.label == "Set privacy mode" && data.get("restart_required").boolean() {
                        self.privacy_restart_pending = true;
                    }
                    if reply.label.starts_with("Package update") {
                        self.state = data.get("state").clone();
                        self.gateway_update = Json::Null;
                        self.gui_update = Json::Null;
                    }
                    if matches!(reply.label.as_str(), "Save provider" | "OAuth provider") {
                        self.provider_panel = 0;
                        self.protocol_menu = false;
                    }
                    if reply.label == "Check gateway update" {
                        self.gateway_update = data.clone();
                    }
                    if reply.label == "Check GUI update" || reply.label == "Update GUI" {
                        self.gui_update = data.clone();
                    }
                    self.notice = if !data.get("message").text().is_empty() {
                        data.get("message").text().into()
                    } else {
                        i18n::message("status.complete", &[("action", human(&reply.label))])
                    };
                    self.failed = false;
                    if reply.label == "Update GUI" && data.get("exit_required").boolean() {
                        self.quit = true;
                    }
                    if reply.refresh
                        && reply.label != "Check gateway update"
                        && reply.label != "Test provider"
                    {
                        self.spawn_refresh();
                        self.busy = true;
                    }
                }
                Err(error) => {
                    self.notice = i18n::message("error.action", &[("error", &error)]);
                    self.failed = true;
                }
            }
            if !self.failed
                && let Some(notice) = self.installation_notice()
            {
                self.notice = notice.into();
            } else if !self.failed && self.privacy_needs_restart() {
                self.notice = tr("privacy.restart_required").into();
            } else if !self.failed && self.usage_restart_pending {
                self.notice = tr("usage.restart_required").into();
            }
        }
        changed
    }
    fn privacy_needs_restart(&self) -> bool {
        let privacy = self.state.get("privacy");
        let mode = value(
            privacy,
            "mode",
            if privacy.get("enabled").boolean() {
                "secrets"
            } else {
                "off"
            },
        );
        self.privacy_restart_pending && mode != "off"
    }
    fn privacy_runtime_notice(&self) -> Option<&'static str> {
        if self.privacy_needs_restart() {
            Some(tr("privacy.restart_required"))
        } else if matches!(
            self.state.get("privacy").get("runtime_state").text(),
            "" | "unknown"
        ) {
            Some(tr("privacy.runtime_unknown"))
        } else {
            None
        }
    }
    fn package_update(&mut self, gui: bool) {
        if self.busy {
            return;
        }
        let binary = if gui {
            self.executable.clone()
        } else {
            core_install::resolve_binary(&self.backend.binary)
        };
        let Some(binary) = binary else {
            self.notice = tr("updates.not_owned").into();
            self.failed = true;
            return;
        };
        self.busy = true;
        self.failed = false;
        self.notice = tr("status.package_wait").into();
        let sender = self.sender.clone();
        let backend = self.backend.clone();
        std::thread::spawn(move || {
            let result = if gui {
                package_update::apply(&binary, &["zcall-gui", "zcall-gui-bin"])
            } else {
                package_update::gateway(&binary)
            };
            let result = result.map(|version| {
                Json::object([
                    ("state".into(), backend.refresh()),
                    (
                        "message".into(),
                        Json::String(i18n::message(
                            if gui {
                                "updates.package_restart"
                            } else {
                                "updates.package_ready"
                            },
                            &[("version", &version)],
                        )),
                    ),
                ])
            });
            let _ = sender.send(Reply {
                label: if gui {
                    "Package update GUI"
                } else {
                    "Package update gateway"
                }
                .into(),
                result,
                refresh: false,
                starts_gateway: false,
            });
        });
    }
    fn self_update(&mut self, apply: bool) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.failed = false;
        self.notice = if apply {
            tr("status.updating_gui")
        } else {
            tr("status.checking_gui")
        }
        .into();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = if apply {
                updater::apply()
            } else {
                updater::check()
            };
            let _ = sender.send(Reply {
                label: if apply {
                    "Update GUI"
                } else {
                    "Check GUI update"
                }
                .into(),
                result,
                refresh: false,
                starts_gateway: false,
            });
        });
    }
    fn draw(&mut self, c: &mut Canvas<'_>, event: Option<Event>) {
        if self.page == USAGE_PAGE && self.usage == Json::Null && !self.busy {
            self.refresh_usage();
        }
        if matches!(event, Some(Event::Key { code: 27, .. })) {
            if self.protocol_menu {
                self.protocol_menu = false;
            } else if self.route_picker.is_some() {
                self.route_picker = None;
                self.ui = Ui::default();
            } else if self.provider_models.take().is_some() || self.palette_editor.take().is_some()
            {
                self.ui = Ui::default();
                self.repaint = true;
            } else {
                self.confirm = None;
                self.provider_panel = 0;
                self.plugin_panel = 0;
                self.service_panel = false;
                self.settings_panel = false;
                self.install_details = false;
                self.clear_key();
            }
        }
        if let Some(Event::Scroll(delta)) = &event {
            if let Some(picker) = self.route_picker.as_mut() {
                picker.scroll = if *delta > 0 {
                    picker.scroll.saturating_add(1)
                } else {
                    picker.scroll.saturating_sub(1)
                };
            } else if let Some(browser) = self.provider_models.as_mut() {
                if *delta > 0 {
                    browser.scroll = browser.scroll.saturating_add(1);
                } else {
                    browser.scroll = browser.scroll.saturating_sub(1);
                }
            } else if !self.modal() {
                self.scroll = if *delta > 0 {
                    self.scroll.saturating_add(1)
                } else {
                    self.scroll.saturating_sub(1)
                };
            }
        }
        self.ui.begin(event);
        let sidebar = 166.;
        c.rect(Rect::new(0., 0., sidebar, c.height), 0., SURFACE);
        brand(c, Rect::new(18., 26., 38., 38.), SURFACE);
        c.text("zcall", 64., 33., 21, true, TEXT);
        c.ellipsis(
            tr("brand.subtitle"),
            22.,
            76.,
            sidebar - 44.,
            12,
            false,
            MUTED,
        );
        let modal = self.modal();
        for (index, label) in PAGES.iter().enumerate() {
            let r = Rect::new(11., 121. + index as f32 * 48., 144., 42.);
            let selected = self.page == index;
            if selected {
                c.rect(r, 6., ACCENT_SOFT);
            }
            if !modal
                && self
                    .ui
                    .button(c, 10 + index as u64, r, "", Style::Quiet, true)
            {
                self.page = index;
                self.scroll = 0;
                self.repaint = true;
            }
            if selected {
                c.rect(Rect::new(r.x + 1., r.y + 13., 3., 16.), 1.5, ACCENT);
            }
            icon(
                c,
                index,
                r.x + 16.,
                r.y + 13.,
                if selected { ACCENT } else { MUTED },
                if selected { ACCENT_SOFT } else { SURFACE },
            );
            c.text(
                tr(label),
                r.x + 45.,
                r.y + 11.,
                16,
                selected,
                if selected { ACCENT } else { TEXT },
            );
        }
        if self.ui.button(
            c,
            40,
            Rect::new(13., c.height - 108., 138., 38.),
            "",
            Style::Quiet,
            !modal,
        ) {
            self.preferences.apply();
            self.settings_panel = true;
        }
        let sy = c.height - 98.;
        for (dy, cx) in [(0., 32.), (7., 39.), (14., 30.)] {
            c.line((27., sy + dy), (43., sy + dy), 1.5, MUTED);
            c.rect(Rect::new(cx - 2., sy + dy - 2., 4., 4.), 2., MUTED);
        }
        c.text(tr("settings.open"), 56., c.height - 98., 15, false, TEXT);
        c.text("Project Z", 23., c.height - 57., 12, false, MUTED);
        c.text(
            &format!("v{VERSION}"),
            23.,
            c.height - 34.,
            12,
            false,
            MUTED,
        );
        let available = (c.width - sidebar - 64.).max(1.);
        let width = available.min(1024.).round();
        let x = (sidebar + 32. + (available - width) / 2.).round();
        if self.page != 0 {
            c.text(tr(PAGES[self.page]), x, 34., 26, true, TEXT);
        }
        let descriptions = [
            tr("page.overview"),
            tr("page.providers"),
            tr("page.agents"),
            tr("page.plugins"),
            tr("page.privacy"),
            tr("page.usage"),
            tr("page.updates"),
        ];
        if self.page != 0 {
            c.ellipsis(descriptions[self.page], x, 82., width, 15, false, MUTED);
        }
        let section = match self.page {
            1 => "providers",
            2 => "agents",
            3 => "plugins",
            4 => "privacy",
            _ => "",
        };
        let section_state = match self.page {
            1..=3 => view_state::collection_state(&self.state, section, section),
            4 => view_state::privacy_state(&self.state),
            _ => LoadState::Ready,
        };
        if section_state == LoadState::Loading {
            empty(
                c,
                Rect::new(x, 140., width, 172.),
                tr("status.loading"),
                tr("status.loading_help"),
            );
        } else if section_state == LoadState::Unavailable {
            card(c, Rect::new(x, 140., width, 194.));
            c.text(tr("error.section"), x + 26., 164., 21, true, TEXT);
            let error = value(
                self.state.get(section),
                "error",
                tr("error.invalid_response"),
            );
            c.ellipsis(error, x + 26., 207., width - 52., 14, false, MUTED);
            if self.ui.button(
                c,
                31,
                Rect::new(x + 26., 266., 132., 40.),
                tr("action.retry"),
                Style::Secondary,
                self.enabled(),
            ) {
                self.refresh();
            }
        } else {
            match self.page {
                0 => self.overview(c, x, width),
                1 => self.providers(c, x, width),
                2 => self.agents(c, x, width),
                3 => self.plugins(c, x, width),
                4 => self.privacy(c, x, width),
                USAGE_PAGE => self.usage_page(c, x, width),
                _ => self.updates(c, x, width),
            }
        }
        c.rect(
            Rect::new(sidebar, c.height - 44., c.width - sidebar, 44.),
            0.,
            SURFACE,
        );
        c.rect(
            Rect::new(x, c.height - 25., 6., 6.),
            3.,
            if self.failed {
                RED
            } else if self.busy {
                ACCENT
            } else {
                MUTED
            },
        );
        c.ellipsis(
            &self.notice,
            x + 17.,
            c.height - 32.,
            width - 103.,
            12,
            false,
            if self.failed { RED } else { MUTED },
        );
        if self.ui.button(
            c,
            30,
            Rect::new(x + width - 70., c.height - 39., 70., 32.),
            tr("action.refresh"),
            Style::Quiet,
            self.enabled(),
        ) {
            self.refresh();
        }
        if self.provider_models.is_some() {
            self.provider_models_dialog(c);
        } else if self.provider_panel > 0 {
            self.provider_dialog(c);
        }
        if self.plugin_panel > 0 {
            self.plugin_dialog(c);
        }
        if self.service_panel {
            self.service_dialog(c);
        }
        if self.palette_editor.is_some() {
            self.palette_dialog(c);
        } else if self.settings_panel {
            self.settings_dialog(c);
        }
        if self.route_picker.is_some() {
            self.agent_route_dialog(c);
        }
        if self.install_details {
            self.installation_dialog(c);
        }
        if self.confirm.is_some() {
            self.confirm_dialog(c);
        }
        self.ui.end();
    }
}
const PROVIDER_AUTH: [&str; 5] = ["default", "bearer", "x-api-key", "x-goog-api-key", "none"];

fn protocols() -> [&'static str; 6] {
    [
        "chat",
        "responses",
        "anthropic",
        "gemini",
        "completions",
        "interactions",
    ]
}
fn protocol_name(index: usize) -> &'static str {
    [
        "OpenAI Chat",
        "OpenAI Responses",
        "Anthropic",
        "Gemini",
        "Completions",
        "Interactions",
    ][index]
}
fn card(c: &mut Canvas<'_>, r: Rect) {
    c.rect(r, 10., SURFACE);
}
fn empty(c: &mut Canvas<'_>, r: Rect, title: &str, detail: &str) {
    c.rect(r, 8., SURFACE);
    c.text(title, r.x + 28., r.y + 31., 22, true, TEXT);
    c.ellipsis(detail, r.x + 28., r.y + 75., r.w - 56., 15, false, MUTED);
}
#[allow(
    clippy::too_many_arguments,
    reason = "form fields carry placement and editable state"
)]
fn field(
    ui: &mut Ui,
    c: &mut Canvas<'_>,
    id: u64,
    x: f32,
    y: f32,
    width: f32,
    label: &str,
    text: &mut String,
    placeholder: &str,
    secret: bool,
    enabled: bool,
) {
    c.text(label, x, y, 14, true, TEXT);
    ui.input(
        c,
        id,
        Rect::new(x, y + 29., width, 46.),
        text,
        placeholder,
        secret,
        enabled,
    );
}
fn brand(c: &mut Canvas<'_>, r: Rect, background: u32) {
    const OUTER: [(f32, f32); 33] = [
        (9.6, 5.0),
        (21.0, 5.0),
        (21.46875, 5.03125),
        (21.875, 5.125),
        (22.21875, 5.28125),
        (22.5, 5.5),
        (22.71875, 5.78125),
        (22.875, 6.125),
        (22.96875, 6.53125),
        (23.0, 7.0),
        (23.0, 9.2),
        (6.4, 20.0),
        (5.5, 20.0),
        (5.03125, 19.96875),
        (4.625, 19.875),
        (4.28125, 19.71875),
        (4.0, 19.5),
        (3.78125, 19.21875),
        (3.625, 18.875),
        (3.53125, 18.46875),
        (3.5, 18.0),
        (3.5, 11.7),
        (3.55318, 10.79091),
        (3.7088, 9.91898),
        (3.96094, 9.09219),
        (4.3037, 8.31852),
        (4.73119, 7.60596),
        (5.2375, 6.9625),
        (5.81672, 6.39612),
        (6.46296, 5.91481),
        (7.17031, 5.52656),
        (7.93287, 5.23935),
        (8.74473, 5.06117),
    ];
    const INNER: [(f32, f32); 13] = [
        (10.0, 10.0),
        (17.0, 10.0),
        (17.0, 13.10361),
        (13.16296, 15.6),
        (8.0, 15.6),
        (8.0, 12.3),
        (8.03125, 11.76094),
        (8.125, 11.29375),
        (8.28125, 10.89844),
        (8.5, 10.575),
        (8.78125, 10.32344),
        (9.125, 10.14375),
        (9.53125, 10.03594),
    ];
    let ink = TEXT;
    for flipped in [false, true] {
        let point = |(x, y): (f32, f32)| {
            let (x, y) = if flipped { (32. - x, 32. - y) } else { (x, y) };
            (r.x + x * r.w / 32., r.y + y * r.h / 32.)
        };
        // Both contours are convex; the cutout matches this surface's background.
        c.polygon(&OUTER.map(point), ink);
        c.polygon(&INNER.map(point), background);
    }
}

fn icon(c: &mut Canvas<'_>, index: usize, x: f32, y: f32, ink: u32, bg: u32) {
    let outline = |c: &mut Canvas<'_>, r: Rect, radius: f32| {
        c.rect(r, radius, ink);
        c.rect(r.inset(1.5), (radius - 1.5).max(0.), bg);
    };
    match index {
        0 => {
            for (dx, dy) in [(0., 0.), (10., 0.), (0., 10.), (10., 10.)] {
                outline(c, Rect::new(x + dx, y + dy, 7., 7.), 2.);
            }
        }
        1 => {
            c.line((x + 4., y + 4.), (x + 13., y + 13.), 1.5, ink);
            outline(c, Rect::new(x, y, 8., 8.), 4.);
            outline(c, Rect::new(x + 9., y + 9., 8., 8.), 4.);
        }
        2 => {
            outline(c, Rect::new(x, y + 1., 18., 15.), 3.);
            c.line((x + 4., y + 5.), (x + 7., y + 8.), 1.5, ink);
            c.line((x + 7., y + 8.), (x + 4., y + 11.), 1.5, ink);
            c.line((x + 10., y + 11.), (x + 14., y + 11.), 1.5, ink);
        }
        3 => {
            for (dx, dy) in [(0., 0.), (10., 0.), (0., 10.), (10., 10.)] {
                outline(
                    c,
                    Rect::new(x + dx, y + dy, 7., 7.),
                    if dx == dy { 2. } else { 3.5 },
                );
            }
        }
        4 => {
            outline(c, Rect::new(x + 4., y, 9., 11.), 4.5);
            outline(c, Rect::new(x + 1., y + 7., 15., 11.), 3.);
            c.line((x + 8.5, y + 11.), (x + 8.5, y + 14.), 1.5, ink);
        }
        USAGE_PAGE => {
            for (dx, height) in [(1., 6.), (7., 11.), (13., 17.)] {
                c.rect(Rect::new(x + dx, y + 18. - height, 3., height), 1.5, ink);
            }
        }
        _ => {
            let mut previous = None;
            for i in 0..=18 {
                let a = (-70. + i as f32 * 300. / 18.).to_radians();
                let p = (x + 8.5 + a.cos() * 7., y + 8.5 + a.sin() * 7.);
                if let Some(from) = previous {
                    c.line(from, p, 1.5, ink);
                }
                previous = Some(p);
            }
            c.line((x + 11., y + 1.), (x + 11., y + 6.), 1.5, ink);
            c.line((x + 11., y + 6.), (x + 16., y + 6.), 1.5, ink);
        }
    }
}

fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut config = None;
    let mut smoke = None;
    let mut preferences = settings::Preferences::load();
    let mut binary = std::env::var("ZCALLD_BIN")
        .ok()
        .or_else(|| {
            preferences
                .core_binary
                .clone()
                .filter(|value| core_install::resolve_binary(value).is_some())
        })
        .or_else(core_install::discover_default)
        .unwrap_or_else(|| "zcalld".into());
    let mut core_operation = None;
    let mut page = 0;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--version" | "-V" => {
                println!("zcall-gui {VERSION}");
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "zcall-gui {VERSION}\n\nUsage: zcall-gui [--zcalld PATH] [--config PATH] [--page overview|providers|agents|plugins|privacy|usage|updates]\n       zcall-gui --update check|apply\n       zcall-gui --core-install prepare|apply|status|resume|rollback\n\n--core-install returns JSON and uses the same verified full-bundle installer as the UI.\n--theme system|light|dark selects appearance for this session.\n--language system|zh-CN|zh-TW|en|ja selects the interface language.\n\nCtrl+Q closes the window. The gateway service stays independent.\n--smoke-seconds N opens a real window for a bounded release smoke test."
                );
                return Ok(());
            }
            "--zcalld" | "--config" | "--smoke-seconds" | "--page" | "--update" | "--theme"
            | "--language" | "--core-install" => {
                let option = args[index].clone();
                index += 1;
                let text = args
                    .get(index)
                    .ok_or_else(|| format!("{option} needs a value"))?;
                match option.as_str() {
                    "--core-install" => {
                        if core_operation.is_some() {
                            return Err("Duplicate --core-install".into());
                        }
                        use core_install_ui::Operation;
                        core_operation = Some(match text.as_str() {
                            "prepare" => Operation::Prepare,
                            "apply" => Operation::Apply,
                            "status" => Operation::Status,
                            "resume" => Operation::Resume,
                            "rollback" => Operation::Rollback,
                            _ => {
                                return Err(
                                    "Use --core-install prepare|apply|status|resume|rollback"
                                        .into(),
                                );
                            }
                        });
                    }
                    "--theme" => {
                        preferences.theme =
                            settings::ThemeChoice::parse(text).ok_or("Unknown theme")?
                    }
                    "--language" => {
                        preferences.language = *i18n::LANGUAGES
                            .iter()
                            .find(|id| **id == text.as_str())
                            .ok_or("Unknown language")?
                    }
                    "--zcalld" => binary = text.clone(),
                    "--config" => config = Some(text.clone()),
                    "--smoke-seconds" => {
                        smoke = Some(
                            text.parse::<u64>()
                                .map_err(|_| "Invalid smoke-test duration")?
                                .clamp(1, 60),
                        )
                    }
                    "--page" => {
                        page = PAGE_IDS
                            .iter()
                            .position(|p| p.eq_ignore_ascii_case(text))
                            .ok_or("Unknown page")?
                    }
                    "--update" => {
                        let result = match text.as_str() {
                            "check" => updater::check(),
                            "apply" => updater::apply(),
                            _ => Err("Use --update check|apply".into()),
                        }?;
                        println!("{}", value_result(&result));
                        return Ok(());
                    }
                    _ => unreachable!(),
                }
            }
            _ => return Err(format!("Unknown option: {}", args[index])),
        }
        index += 1;
    }
    if let Some(operation) = core_operation {
        let result = operation.perform(&binary)?;
        if matches!(
            operation,
            core_install_ui::Operation::Apply | core_install_ui::Operation::Resume
        ) && core_install_ui::committed(&result)
        {
            preferences.core_binary = Some(result.get("binary_path").text().into());
            preferences.save()?;
        }
        println!("{}", result.stringify());
        if result != Json::Null && !result.get("ok").boolean() {
            return Err(
                "Installation did not complete; its saved transaction is available for review."
                    .into(),
            );
        }
        return Ok(());
    }
    preferences.apply();
    let mut window = Window::new(tr("window.title"), 1080, 740)?;
    let gpu = window.gpu();
    eprintln!("zcall-gui {VERSION}; renderer: {gpu}");
    let mut app = App::new(Backend { binary, config }, preferences);
    app.page = page;
    let started = Instant::now();
    let mut event = None;
    let mut dirty = true;
    let mut frames = 0u64;
    let mut smoke_ready = false;
    loop {
        if app.quit
            || matches!(
                event,
                Some(
                    Event::Close
                        | Event::Key {
                            code: 81 | 113,
                            control: true,
                            ..
                        }
                )
            )
            || smoke.is_some_and(|n| started.elapsed() >= Duration::from_secs(n))
        {
            break;
        }
        dirty |= app.poll();
        if dirty || event.is_some() {
            let theme = app.preferences.resolved_theme();
            window.set_theme(theme);
            let colors = app
                .palette_editor
                .as_ref()
                .map(|editor| editor.preview)
                .unwrap_or_else(|| app.preferences.colors(theme));
            window.set_palette(colors.palette());
            {
                let mut c = window.begin(BG);
                app.draw(&mut c, event.take());
            }
            window.present();
            frames += 1;
            if smoke.is_some() && !app.busy && !smoke_ready {
                eprintln!("Smoke ready: state loaded and presented");
                smoke_ready = true;
            }
            if let Some(text) = app.ui.clipboard.take() {
                window.copy(&text);
            }
            dirty = std::mem::take(&mut app.repaint);
            if dirty {
                continue;
            }
        }
        event = window.event(if app.busy { 100 } else { 1000 });
        if let Some(Event::Move(x, y)) = event.as_ref()
            && !app.ui.pointer_changed(*x, *y)
        {
            event = None;
        }
    }
    if smoke.is_some() {
        eprintln!("Presented frames: {frames}");
    }
    Ok(())
}
fn value_result(value: &Json) -> String {
    if !value.get("message").text().is_empty() {
        value.get("message").text().into()
    } else {
        format!(
            "Installed: {VERSION}; latest: {}",
            value.get("latest").text()
        )
    }
}
fn main() {
    if let Err(error) =
        updater::complete_if_requested().and_then(|done| if done { Ok(()) } else { run() })
    {
        eprintln!("zcall-gui: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod privacy_state_tests {
    use super::*;
    fn app() -> App {
        let mut app = App::unloaded(
            Backend {
                binary: "never-executed-zcalld".into(),
                config: None,
            },
            settings::Preferences::test_defaults(),
        );
        app.busy = false;
        app
    }
    fn reply(app: &mut App, label: &str, body: &str, starts_gateway: bool) {
        app.sender
            .send(Reply {
                label: label.into(),
                result: json::parse(body),
                refresh: false,
                starts_gateway,
            })
            .unwrap();
        assert!(app.poll());
    }
    fn refresh(app: &mut App) {
        reply(
            app,
            "refresh",
            r#"{"status":{"healthy":true},"privacy":{"enabled":true,"mode":"secrets","restart_required":null}}"#,
            false,
        );
    }
    #[test]
    fn privacy_pending_survives_refresh_without_restart_field() {
        let mut app = app();
        refresh(&mut app);
        reply(
            &mut app,
            "Set privacy mode",
            r#"{"ok":true,"restart_required":true}"#,
            false,
        );
        refresh(&mut app);
        assert!(app.privacy_needs_restart());
        assert_eq!(app.notice, tr("privacy.restart_required"));
        reply(
            &mut app,
            "Set privacy mode",
            r#"{"ok":true,"restart_required":false}"#,
            false,
        );
        refresh(&mut app);
        assert!(app.privacy_needs_restart());
    }
    #[test]
    fn successful_gateway_start_or_restart_clears_pending_after_refresh() {
        for label in [
            "启动网关",
            "重新啟動閘道",
            "ゲートウェイ再起動",
            "Restart gateway",
        ] {
            let mut app = app();
            refresh(&mut app);
            reply(
                &mut app,
                "Set privacy mode",
                r#"{"restart_required":true}"#,
                false,
            );
            reply(&mut app, label, r#"{"ok":true}"#, true);
            refresh(&mut app);
            assert!(!app.privacy_restart_pending);
            assert!(!app.privacy_needs_restart());
            assert_eq!(app.notice, tr("status.refreshed"));
        }
    }
    #[test]
    fn reopening_with_unknown_runtime_never_claims_saved_mode_is_running() {
        let mut original = app();
        refresh(&mut original);
        reply(
            &mut original,
            "Set privacy mode",
            r#"{"restart_required":true}"#,
            false,
        );
        assert_eq!(
            original.privacy_runtime_notice(),
            Some(tr("privacy.restart_required"))
        );
        drop(original);
        let mut reopened = app();
        reply(
            &mut reopened,
            "refresh",
            r#"{"status":{"healthy":true},"privacy":{"configured":true,"enabled":true,"mode":"secrets","runtime_state":"unknown","restart_required":null}}"#,
            false,
        );
        assert!(!reopened.privacy_restart_pending);
        assert_eq!(
            reopened.privacy_runtime_notice(),
            Some(tr("privacy.runtime_unknown"))
        );
        refresh(&mut reopened);
        assert_eq!(
            reopened.privacy_runtime_notice(),
            Some(tr("privacy.runtime_unknown"))
        );
    }
    #[test]
    fn failed_restart_keeps_pending_and_error() {
        let mut app = app();
        refresh(&mut app);
        reply(
            &mut app,
            "Set privacy mode",
            r#"{"restart_required":true}"#,
            false,
        );
        app.sender
            .send(Reply {
                label: "Restart gateway".into(),
                result: Err("service failed".into()),
                refresh: false,
                starts_gateway: true,
            })
            .unwrap();
        assert!(app.poll());
        assert!(app.privacy_needs_restart());
        assert!(app.failed);
        assert!(app.notice.contains("service failed"));
    }
}

#[cfg(test)]
mod agent_route_tests {
    use super::*;
    fn app(catalog: &str) -> App {
        let mut app = App::unloaded(
            Backend {
                binary: "never-executed-zcalld".into(),
                config: None,
            },
            settings::Preferences::test_defaults(),
        );
        app.busy = false;
        app.state = json::parse(catalog).unwrap();
        app
    }
    fn agent(route: &str) -> Json {
        Json::object([
            ("name".into(), Json::String("claude".into())),
            ("route".into(), Json::String(route.into())),
            ("installed".into(), Json::Bool(true)),
            ("writable".into(), Json::Bool(true)),
        ])
    }
    #[test]
    fn real_catalog_ids_are_used_and_search_matches_labels() {
        let app = app(
            r#"{"models":{"object":"list","data":[{"id":"team/writing","name":"Writing model"},{"id":"team/code","name":"Coding"},{"id":"team/code"},{"name":"not-a-route"}]}}"#,
        );
        let routes = app.route_choices();
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].id, "team/code");
        assert!(routes[1].matches("WRITING"));
        assert!(routes[1].matches("team/"));
    }
    #[test]
    fn existing_route_is_selected_only_while_it_is_available() {
        let mut app = app(r#"{"models":{"data":[{"id":"team/writing"}]}}"#);
        app.open_route_picker(&agent("team/writing"));
        assert_eq!(app.route_picker.as_ref().unwrap().selected, "team/writing");
        assert!(app.modal());
        assert!(!app.enabled());
        app.route_picker = None;
        app.open_route_picker(&agent("removed-route"));
        assert!(app.route_picker.as_ref().unwrap().selected.is_empty());
        assert_eq!(app.route_picker.as_ref().unwrap().current, "removed-route");
    }
    #[test]
    fn missing_or_failed_catalog_never_invents_a_local_route() {
        for state in [
            r#"{"models":{"data":[]}}"#,
            r#"{"models":{"error":"unavailable"}}"#,
        ] {
            let mut app = app(state);
            app.open_route_picker(&agent("local"));
            assert!(app.route_choices().is_empty());
            assert!(app.route_picker.is_none());
            assert!(app.confirm.is_none());
        }
    }
    #[test]
    fn detection_only_agents_cannot_open_an_apply_picker() {
        let mut app = app(r#"{"models":{"data":[{"id":"team/writing"}]}}"#);
        let readonly =
            json::parse(r#"{"name":"cursor","installed":true,"writable":false}"#).unwrap();
        app.open_route_picker(&readonly);
        assert!(app.route_picker.is_none());
    }
}
