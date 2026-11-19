#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod backend;
mod dialogs;
mod i18n;
mod json;
mod package_update;
mod pages;
mod settings;
mod updater;
use backend::{Backend, Reply};
use i18n::tr;
use json::Json;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use zui::{ButtonStyle as Style, Canvas, Event, Rect, Ui, Window, colors::*};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const PAGES: [&str; 6] = [
    "nav.overview",
    "nav.providers",
    "nav.agents",
    "nav.plugins",
    "nav.privacy",
    "nav.updates",
];
const PAGE_IDS: [&str; 6] = [
    "overview",
    "providers",
    "agents",
    "plugins",
    "privacy",
    "updates",
];
#[derive(Default)]
struct ProviderForm {
    name: String,
    url: String,
    model: String,
    environment: String,
    key: String,
    protocol: usize,
}
#[derive(Default)]
struct PluginForm {
    source: String,
    revision: String,
    name: String,
    sha256: String,
}
struct Action {
    label: String,
    args: Vec<String>,
    secret: Option<Vec<u8>>,
}
struct App {
    backend: Backend,
    ui: Ui,
    page: usize,
    scroll: usize,
    state: Json,
    busy: bool,
    notice: String,
    failed: bool,
    sender: mpsc::Sender<Reply>,
    receiver: mpsc::Receiver<Reply>,
    provider: ProviderForm,
    plugin: PluginForm,
    route: String,
    confirm: Option<Action>,
    gateway_update: Json,
    gui_update: Json,
    quit: bool,
    provider_panel: u8,
    provider_edit: bool,
    plugin_panel: u8,
    selected_plugin: Json,
    service_panel: bool,
    protocol_menu: bool,
    preferences: settings::Preferences,
    settings_panel: bool,
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
        "Save provider" => tr("action.save_provider"),
        "Test provider" => tr("action.test_provider"),
        "Detect AI tools" => tr("action.detect_agents"),
        "Set privacy mode" => tr("action.privacy"),
        "Check gateway update" => tr("action.check_gateway"),
        "Check GUI update" => tr("action.check_gui"),
        "Update GUI" => tr("action.update_gui"),
        "Start gateway" => tr("action.start_gateway"),
        "Restart gateway" => tr("action.restart_gateway"),
        "Stop gateway" => tr("action.stop_gateway"),
        _ => s,
    }
}
impl App {
    fn new(backend: Backend, preferences: settings::Preferences) -> Self {
        let app = Self::unloaded(backend, preferences);
        app.backend.spawn_refresh(app.sender.clone());
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
            busy: true,
            notice: tr("status.loading").into(),
            failed: false,
            sender,
            receiver,
            provider: ProviderForm::default(),
            plugin: PluginForm::default(),
            route: String::new(),
            confirm: None,
            gateway_update: Json::Null,
            gui_update: Json::Null,
            quit: false,
            provider_panel: 0,
            provider_edit: false,
            plugin_panel: 0,
            selected_plugin: Json::Null,
            service_panel: false,
            protocol_menu: false,
            preferences,
            settings_panel: false,
            repaint: false,
            executable: std::env::current_exe().ok(),
            privacy_restart_pending: false,
        }
    }
    fn modal(&self) -> bool {
        self.confirm.is_some()
            || self.provider_panel > 0
            || self.plugin_panel > 0
            || self.service_panel
            || self.settings_panel
    }
    fn enabled(&self) -> bool {
        !self.busy && !self.modal()
    }
    fn refresh(&mut self) {
        if !self.busy {
            self.busy = true;
            self.notice = tr("status.loading").into();
            self.failed = false;
            self.backend.spawn_refresh(self.sender.clone());
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
        });
    }
    fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(reply) = self.receiver.try_recv() {
            changed = true;
            self.busy = false;
            match reply.result {
                Ok(data) if reply.label == "refresh" => {
                    self.state = data;
                    let error = self.state.get("status").get("error").text();
                    if !error.is_empty() {
                        self.notice = i18n::message("error.connection", &[("error", error)]);
                        self.failed = true;
                    } else {
                        self.notice = tr("status.refreshed").into();
                        self.failed = false;
                    }
                }
                Ok(data) => {
                    if reply.starts_gateway {
                        self.privacy_restart_pending = false;
                    }
                    if reply.label == "Set privacy mode" && data.get("restart_required").boolean() {
                        self.privacy_restart_pending = true;
                    }
                    if reply.label.starts_with("Package update") {
                        self.state = data.get("state").clone();
                        self.gateway_update = Json::Null;
                        self.gui_update = Json::Null;
                    }
                    if reply.label == "Save provider" {
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
                        self.backend.spawn_refresh(self.sender.clone());
                        self.busy = true;
                    }
                }
                Err(error) => {
                    self.notice = i18n::message("error.action", &[("error", &error)]);
                    self.failed = true;
                }
            }
            if !self.failed && self.privacy_needs_restart() {
                self.notice = tr("privacy.restart_required").into();
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
            Some(std::path::PathBuf::from(&self.backend.binary))
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
            let owners = if gui {
                &["zcall-gui", "zcall-gui-bin"]
            } else {
                &["zcalld", "zcalld-bin"]
            };
            let result = package_update::apply(&binary, owners).map(|version| {
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
        if matches!(event, Some(Event::Key { code: 27, .. })) {
            if self.protocol_menu {
                self.protocol_menu = false;
            } else {
                self.confirm = None;
                self.provider_panel = 0;
                self.plugin_panel = 0;
                self.service_panel = false;
                self.settings_panel = false;
                self.clear_key();
            }
        }
        if let Some(Event::Scroll(delta)) = &event
            && !self.modal()
        {
            self.scroll = if *delta > 0 {
                self.scroll.saturating_add(1)
            } else {
                self.scroll.saturating_sub(1)
            };
        }
        self.ui.begin(event);
        let sidebar = 166.;
        c.rect(Rect::new(0., 0., sidebar, c.height), 0., 0xf7f9fcff);
        c.rect(Rect::new(sidebar - 1., 0., 1., c.height), 0., 0xe8edf3ff);
        brand(c, Rect::new(18., 26., 38., 38.), BG);
        c.text("zcall", 64., 33., 21, true, TEXT);
        c.text(tr("brand.subtitle"), 22., 76., 12, false, MUTED);
        let modal = self.modal();
        for (index, label) in PAGES.iter().enumerate() {
            let r = Rect::new(11., 121. + index as f32 * 48., 144., 42.);
            let selected = self.page == index;
            if selected {
                c.rect(r, 12., BLUE_PALE);
            }
            if self
                .ui
                .button(c, 10 + index as u64, r, "", Style::Quiet, !modal)
            {
                self.page = index;
                self.scroll = 0;
            }
            if selected {
                c.rect(Rect::new(r.x + 1., r.y + 13., 3., 16.), 1.5, BLUE);
            }
            icon(
                c,
                index,
                r.x + 16.,
                r.y + 13.,
                if selected { BLUE } else { MUTED },
                if selected { BLUE_PALE } else { 0xf7f9fcff },
            );
            c.text(
                tr(label),
                r.x + 45.,
                r.y + 11.,
                16,
                selected,
                if selected { BLUE } else { TEXT },
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
        let available = (c.width - sidebar - 64.).max(480.);
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
            tr("page.updates"),
        ];
        if self.page != 0 {
            c.text(descriptions[self.page], x, 82., 15, false, MUTED);
        }
        let section = match self.page {
            1 => "providers",
            2 => "agents",
            3 => "plugins",
            4 => "privacy",
            _ => "",
        };
        let error = self.state.get(section).get("error").text().to_owned();
        if !error.is_empty() {
            card(c, Rect::new(x, 140., width, 164.));
            c.text(tr("error.section"), x + 26., 164., 21, true, TEXT);
            c.ellipsis(&error, x + 26., 207., width - 52., 14, false, RED);
            c.text(tr("error.section_help"), x + 26., 252., 14, false, MUTED);
        } else {
            match self.page {
                0 => self.overview(c, x, width),
                1 => self.providers(c, x, width),
                2 => self.agents(c, x, width),
                3 => self.plugins(c, x, width),
                4 => self.privacy(c, x, width),
                _ => self.updates(c, x, width),
            }
        }
        c.rect(
            Rect::new(sidebar, c.height - 44., c.width - sidebar, 44.),
            0.,
            SURFACE,
        );
        c.rect(Rect::new(x, c.height - 44., width, 1.), 0., LINE);
        c.rect(
            Rect::new(x, c.height - 25., 6., 6.),
            3.,
            if self.failed {
                RED
            } else if self.busy {
                BLUE
            } else {
                0x9aa0a6ff
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
        if self.provider_panel > 0 {
            self.provider_dialog(c);
        }
        if self.plugin_panel > 0 {
            self.plugin_dialog(c);
        }
        if self.service_panel {
            self.service_dialog(c);
        }
        if self.settings_panel {
            self.settings_dialog(c);
        }
        if self.confirm.is_some() {
            self.confirm_dialog(c);
        }
        self.ui.end();
    }
}
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
    c.rect(r, 16., LINE);
    c.rect(r.inset(1.), 15., SURFACE);
}
fn empty(c: &mut Canvas<'_>, r: Rect, title: &str, detail: &str) {
    c.rect(r, 18., 0xf7f9fcff);
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
    let ink = if c.theme == zui::Theme::Dark {
        0x8bb7ffff
    } else {
        0x1769f5ff
    };
    let point = |(x, y): (f32, f32)| (r.x + x * r.w / 64., r.y + y * r.h / 64.);
    let radius = 5. * r.w / 64.;
    let mut points = vec![(13., 17.), (48., 17.)];
    for i in 1..=8 {
        let t = i as f32 / 8.;
        let u = 1. - t;
        points.push((
            u * u * u * 48. + 3. * u * u * t * 51.6 + 3. * u * t * t * 53.4 + t * t * t * 50.8,
            u * u * u * 17. + 3. * u * u * t * 17. + 3. * u * t * t * 21.4 + t * t * t * 24.,
        ));
    }
    points.extend([(23.2, 47.), (51., 47.)]);
    for pair in points.windows(2) {
        c.line(point(pair[0]), point(pair[1]), radius * 2., ink);
    }
    for p in points {
        let (x, y) = point(p);
        c.rect(
            Rect::new(x - radius, y - radius, radius * 2., radius * 2.),
            radius,
            ink,
        );
    }
    for p in [(13., 17.), (51., 47.)] {
        let (x, y) = point(p);
        let hole = 3. * r.w / 64.;
        c.rect(
            Rect::new(x - hole, y - hole, hole * 2., hole * 2.),
            hole,
            background,
        );
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
    let mut binary = std::env::var("ZCALLD_BIN").unwrap_or_else(|_| "zcalld".into());
    let mut config = None;
    let mut smoke = None;
    let mut preferences = settings::Preferences::load();
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
                    "zcall-gui {VERSION}\n\nUsage: zcall-gui [--zcalld PATH] [--config PATH] [--page overview|providers|agents|plugins|privacy|updates]\n       zcall-gui --update check|apply\n\n--theme system|light|dark selects appearance for this session.\n--language system|zh-CN|zh-TW|en|ja selects the interface language.\n\nCtrl+Q closes the window. The gateway service stays independent.\n--smoke-seconds N opens a real window for a bounded release smoke test."
                );
                return Ok(());
            }
            "--zcalld" | "--config" | "--smoke-seconds" | "--page" | "--update" | "--theme"
            | "--language" => {
                let option = args[index].clone();
                index += 1;
                let text = args
                    .get(index)
                    .ok_or_else(|| format!("{option} needs a value"))?;
                match option.as_str() {
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
            window.set_theme(app.preferences.resolved_theme());
            {
                let mut c = window.begin(SURFACE);
                app.draw(&mut c, event.take());
            }
            window.present();
            frames += 1;
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
