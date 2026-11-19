#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod backend;
mod json;
mod pages;
mod updater;
use backend::{Backend, Reply};
use json::Json;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use zui::{ButtonStyle as Style, Canvas, Event, Rect, Ui, Window, colors::*};
const VERSION: &str = env!("CARGO_PKG_VERSION");
const PAGES: [&str; 6] = [
    "Overview",
    "Providers",
    "Agents",
    "Plugins",
    "Privacy",
    "Updates",
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
    gpu: String,
    quit: bool,
}
fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).into()).collect()
}
fn count(state: &Json, section: &str) -> usize {
    state.get(section).get(section).array().len()
}
fn value<'a>(json: &'a Json, key: &str, fallback: &'a str) -> &'a str {
    let text = json.get(key).text();
    if text.is_empty() { fallback } else { text }
}
impl App {
    fn new(backend: Backend, gpu: String) -> Self {
        let (sender, receiver) = mpsc::channel();
        backend.spawn_refresh(sender.clone());
        Self {
            backend,
            ui: Ui::default(),
            page: 0,
            scroll: 0,
            state: Json::Null,
            busy: true,
            notice: "Reading local gateway state…".into(),
            failed: false,
            sender,
            receiver,
            provider: ProviderForm::default(),
            plugin: PluginForm::default(),
            route: String::new(),
            confirm: None,
            gateway_update: Json::Null,
            gui_update: Json::Null,
            gpu,
            quit: false,
        }
    }
    fn refresh(&mut self) {
        if !self.busy {
            self.busy = true;
            self.notice = "Reading local gateway state…".into();
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
        self.notice = format!("{label}…");
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
                        self.notice = error.to_owned();
                        self.failed = true;
                    } else {
                        self.notice = "Local state is up to date.".into();
                        self.failed = false;
                    }
                }
                Ok(data) => {
                    if reply.label == "Check gateway update" {
                        self.gateway_update = data.clone();
                    }
                    if reply.label == "Check GUI update" || reply.label == "Update GUI" {
                        self.gui_update = data.clone();
                    }
                    self.notice = if !data.get("message").text().is_empty() {
                        data.get("message").text().into()
                    } else {
                        format!("{} completed.", reply.label)
                    };
                    self.failed = false;
                    if reply.label == "Update GUI" && data.get("exit_required").boolean() {
                        self.quit = true;
                    }
                    if reply.refresh && reply.label != "Check gateway update" {
                        self.backend.spawn_refresh(self.sender.clone());
                        self.busy = true;
                    }
                }
                Err(error) => {
                    self.notice = error;
                    self.failed = true;
                }
            }
        }
        changed
    }
    fn self_update(&mut self, apply: bool) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.failed = false;
        self.notice = if apply {
            "Updating GUI…"
        } else {
            "Checking GUI release…"
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
            });
        });
    }
    fn enabled(&self) -> bool {
        !self.busy && self.confirm.is_none()
    }
    fn draw(&mut self, canvas: &mut Canvas<'_>, event: Option<Event>) {
        if let Some(Event::Scroll(delta)) = &event {
            self.scroll = if *delta > 0 {
                self.scroll.saturating_add(1)
            } else {
                self.scroll.saturating_sub(1)
            };
        }
        self.ui.begin(event);
        canvas.rect(Rect::new(0., 0., 184., canvas.height), 0., SURFACE);
        canvas.rect(Rect::new(183., 0., 1., canvas.height), 0., LINE);
        canvas.rect(Rect::new(22., 31., 32., 32.), 8., BLUE);
        canvas.text("z", 31., 32., 23, true, SURFACE);
        canvas.text("zcall", 64., 32., 23, true, TEXT);
        canvas.text("GATEWAY CONTROL", 24., 79., 11, true, MUTED);
        for (index, page) in PAGES.iter().enumerate() {
            if self.page == index {
                canvas.rect(
                    Rect::new(13., 124. + index as f32 * 49., 158., 42.),
                    8.,
                    BLUE_PALE,
                );
            }
            if self.ui.button(
                canvas,
                10 + index as u64,
                Rect::new(13., 124. + index as f32 * 49., 158., 42.),
                page,
                Style::Quiet,
                self.confirm.is_none(),
            ) {
                self.page = index;
                self.scroll = 0;
            }
        }
        canvas.text("PROJECT Z", 24., canvas.height - 88., 11, true, MUTED);
        canvas.text(
            &format!("GUI {VERSION}"),
            24.,
            canvas.height - 63.,
            13,
            false,
            MUTED,
        );
        canvas.text(
            "Native · on demand",
            24.,
            canvas.height - 41.,
            12,
            false,
            MUTED,
        );
        let x = 216.;
        let width = (canvas.width - x - 32.).max(480.);
        canvas.text(PAGES[self.page], x, 35., 29, true, TEXT);
        let descriptions = [
            "Your local AI gateway, in one quiet place.",
            "Connect a provider. Keep credentials on this computer.",
            "Point your AI tools at the gateway, with a reversible change.",
            "Install only what you need. Enable each plugin explicitly.",
            "Choose which values are sanitized before requests leave.",
            "Update the gateway and this optional control panel.",
        ];
        canvas.text(descriptions[self.page], x, 82., 15, false, MUTED);
        if self.ui.button(
            canvas,
            30,
            Rect::new(canvas.width - 132., 37., 100., 38.),
            "Refresh",
            Style::Secondary,
            self.enabled(),
        ) {
            self.refresh();
        }
        let section = match self.page { 1 => "providers", 2 => "agents", 3 => "plugins", 4 => "privacy", _ => "" };
        let section_error = self.state.get(section).get("error").text().to_owned();
        if !section_error.is_empty() {
            card(canvas, Rect::new(x, 130., width, 156.));
            canvas.text("This section is unavailable", x + 24., 151., 21, true, RED);
            canvas.ellipsis(&section_error, x + 24., 196., width - 48., 14, false, TEXT);
            canvas.text("Check the gateway settings or version, then Refresh.", x + 24., 237., 14, false, MUTED);
        } else { match self.page {
            0 => self.overview(canvas, x, width),
            1 => self.providers(canvas, x, width),
            2 => self.agents(canvas, x, width),
            3 => self.plugins(canvas, x, width),
            4 => self.privacy(canvas, x, width),
            _ => self.updates(canvas, x, width),
        }}
        canvas.rect(
            Rect::new(184., canvas.height - 48., canvas.width - 184., 48.),
            0.,
            SURFACE,
        );
        canvas.rect(
            Rect::new(184., canvas.height - 48., canvas.width - 184., 1.),
            0.,
            LINE,
        );
        canvas.rect(
            Rect::new(214., canvas.height - 28., 7., 7.),
            3.5,
            if self.failed {
                RED
            } else if self.busy {
                BLUE
            } else {
                GREEN
            },
        );
        canvas.ellipsis(
            &self.notice,
            234.,
            canvas.height - 34.,
            canvas.width - 264.,
            13,
            false,
            if self.failed { RED } else { MUTED },
        );
        if self.confirm.is_some() {
            canvas.rect(
                Rect::new(0., 0., canvas.width, canvas.height),
                0.,
                0x18233366,
            );
            let dialog = Rect::new(
                (canvas.width - 500.) / 2.,
                (canvas.height - 210.) / 2.,
                500.,
                210.,
            );
            canvas.rect(dialog, 12., SURFACE);
            let title = self.confirm.as_ref().unwrap().label.clone();
            canvas.ellipsis(&title, dialog.x + 28., dialog.y + 26., 444., 22, true, TEXT);
            canvas.text(
                "This changes your local gateway configuration.",
                dialog.x + 28.,
                dialog.y + 72.,
                15,
                false,
                MUTED,
            );
            canvas.text(
                "You can review the result in the CLI afterwards.",
                dialog.x + 28.,
                dialog.y + 98.,
                15,
                false,
                MUTED,
            );
            if self.ui.button(
                canvas,
                9001,
                Rect::new(dialog.x + 226., dialog.y + 145., 110., 40.),
                "Cancel",
                Style::Secondary,
                true,
            ) {
                self.confirm = None;
            }
            if self.ui.button(
                canvas,
                9002,
                Rect::new(dialog.x + 348., dialog.y + 145., 124., 40.),
                "Confirm",
                Style::Primary,
                true,
            ) && let Some(action) = self.confirm.take()
            {
                self.action(&action.label, action.args, action.secret);
            }
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
fn card(c: &mut Canvas<'_>, r: Rect) {
    c.rect(r, 11., LINE);
    c.rect(r.inset(1.), 10., SURFACE);
}
fn empty(c: &mut Canvas<'_>, x: f32, y: f32, width: f32, title: &str, detail: &str) {
    c.text(title, x, y, 20, true, TEXT);
    c.ellipsis(detail, x, y + 39., width, 14, false, MUTED);
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
    c.text(label, x, y, 12, true, MUTED);
    ui.input(
        c,
        id,
        Rect::new(x, y + 23., width, 42.),
        text,
        placeholder,
        secret,
        enabled,
    );
}
fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut binary = std::env::var("ZCALLD_BIN").unwrap_or_else(|_| "zcalld".into());
    let mut config = None;
    let mut smoke = None;
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
                    "zcall-gui {VERSION}\n\nUsage: zcall-gui [--zcalld PATH] [--config PATH] [--page overview|providers|agents|plugins|privacy|updates]\n       zcall-gui --update check|apply\n\nThe GUI calls zcalld --json. It does not start the gateway automatically.\n--smoke-seconds N opens a real window for a bounded release smoke test."
                );
                return Ok(());
            }
            "--zcalld" | "--config" | "--smoke-seconds" | "--page" | "--update" => {
                let option = args[index].clone();
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{option} needs a value"))?;
                match option.as_str() {
                    "--zcalld" => binary = value.clone(),
                    "--config" => config = Some(value.clone()),
                    "--smoke-seconds" => {
                        smoke = Some(
                            value
                                .parse::<u64>()
                                .map_err(|_| "Invalid smoke-test duration")?
                                .clamp(1, 60),
                        )
                    }
                    "--page" => {
                        page = PAGES
                            .iter()
                            .position(|p| p.eq_ignore_ascii_case(value))
                            .ok_or("Unknown page")?
                    }
                    "--update" => {
                        let result = match value.as_str() {
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
    let mut window = Window::new("zcall-gui — Gateway control", 1100, 740)?;
    let gpu = window.gpu();
    eprintln!("zcall-gui {VERSION}; renderer: {gpu}");
    let mut app = App::new(Backend { binary, config }, gpu);
    app.page = page;
    let started = Instant::now();
    let mut event = None;
    let mut dirty = true;
    loop {
        if app.quit
            || matches!(event, Some(Event::Close | Event::Key { code: 81 | 113, control: true, .. }))
            || smoke.is_some_and(|seconds| started.elapsed() >= Duration::from_secs(seconds))
        {
            break;
        }
        dirty |= app.poll();
        if dirty || event.is_some() {
            {
                let mut canvas = window.begin(BG);
                app.draw(&mut canvas, event.take());
            }
            window.present();
            if let Some(text) = app.ui.clipboard.take() {
                window.copy(&text);
            }
            dirty = false;
        }
        event = window.event(if app.busy { 100 } else { 1000 });
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
