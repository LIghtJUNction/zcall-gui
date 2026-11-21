//! Local UI state for complete gateway installation. It never owns extraction.
use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operation {
    Prepare,
    Apply,
    Status,
    Resume,
    Rollback,
}
impl Operation {
    fn id(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::Apply => "apply",
            Self::Status => "status",
            Self::Resume => "resume",
            Self::Rollback => "rollback",
        }
    }
    pub(super) fn perform(self, binary: &str) -> Result<Json, String> {
        match self {
            Self::Prepare => core_install::prepare(binary),
            Self::Apply => core_install::apply(),
            Self::Status => core_install::status(),
            Self::Resume => core_install::recover(false),
            Self::Rollback => core_install::recover(true),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SetupOperation {
    Initialize,
    RegisterService,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SetupStep {
    Initialize,
    RegisterService,
    Ready,
    Inspect,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SetupContext {
    binary: String,
    config: String,
}
impl SetupOperation {
    fn label(self) -> &'static str {
        match self {
            Self::Initialize => "Bootstrap initialize",
            Self::RegisterService => "Bootstrap service install",
        }
    }
    pub(super) fn title(self) -> &'static str {
        tr(match self {
            Self::Initialize => "setup.initialize",
            Self::RegisterService => "setup.install_service",
        })
    }
}
fn selected_installation(info: &Json, binary: &str) -> bool {
    use std::path::Path;
    let prefix = Path::new(info.get("prefix").text());
    let expected = prefix.join("bin").join(if cfg!(windows) {
        "zcalld.exe"
    } else {
        "zcalld"
    });
    committed(info)
        && prefix.is_absolute()
        && Path::new(binary) == expected
        && info.get("binary_path").text() == binary
}
fn context(info: &Json, binary: &str, status: &Json) -> Option<SetupContext> {
    let config = status.get("configuration_path").text();
    if !selected_installation(info, binary)
        || !std::path::Path::new(config).is_absolute()
        || config.len() > 4096
        || config.chars().any(char::is_control)
    {
        return None;
    }
    Some(SetupContext {
        binary: binary.into(),
        config: config.into(),
    })
}
fn setup_step(status: &Json, verified: bool) -> SetupStep {
    match (status.get("configured"), status.get("configuration_exists")) {
        (Json::Bool(false), Json::Bool(false)) => SetupStep::Initialize,
        (Json::Bool(true), Json::Bool(true)) => match status.get("service_install_supported") {
            Json::Bool(true) if !verified => SetupStep::RegisterService,
            Json::Bool(true) => SetupStep::Ready,
            Json::Bool(false) if !cfg!(target_os = "linux") => SetupStep::Ready,
            _ => SetupStep::Inspect,
        },
        _ => SetupStep::Inspect,
    }
}
fn setup_result_context(data: &Json) -> Option<SetupContext> {
    Some(SetupContext {
        binary: match data.get("setup_binary") {
            Json::String(v) => v.clone(),
            _ => return None,
        },
        config: match data.get("setup_config") {
            Json::String(v) => v.clone(),
            _ => return None,
        },
    })
}

pub(super) fn phase(info: &Json) -> &str {
    info.get("phase").text()
}
pub(super) fn committed(info: &Json) -> bool {
    info.get("ok") == &Json::Bool(true)
        && phase(info) == "committed"
        && info.get("committed").boolean()
        && !info.get("pending_exit").boolean()
        && !info.get("helper_started").boolean()
}
pub(super) fn waiting(info: &Json) -> bool {
    info.get("pending_exit").boolean()
        || info.get("helper_started").boolean()
        || matches!(
            phase(info),
            "pending_exit"
                | "rollback_pending"
                | "pending_operation"
                | "preparing"
                | "prepared"
                | "activating"
                | "old_saved"
                | "installed"
        )
}
fn notice(info: &Json) -> Option<&'static str> {
    if waiting(info) {
        Some(tr(if phase(info) == "pending_operation" {
            "install.in_progress"
        } else {
            "install.waiting_help"
        }))
    } else if committed(info) {
        Some(tr("install.completed_help"))
    } else {
        match phase(info) {
            "planned" => Some(tr("install.plan_ready")),
            "rolled_back" => Some(tr("install.rolled_back")),
            "failed" | "needs_attention" => Some(tr("install.recovery_help")),
            _ => None,
        }
    }
}

impl App {
    fn setup_context(&self) -> Option<SetupContext> {
        let context = context(
            &self.core_installation,
            &self.backend.binary,
            self.state.get("status"),
        )?;
        if self
            .backend
            .config
            .as_deref()
            .is_some_and(|selected| selected != context.config)
        {
            return None;
        }
        Some(context)
    }
    pub(super) fn setup_step(&self) -> Option<SetupStep> {
        if !selected_installation(&self.core_installation, &self.backend.binary) {
            return None;
        }
        let Some(context) = self.setup_context() else {
            return Some(SetupStep::Inspect);
        };
        Some(setup_step(
            self.state.get("status"),
            self.setup_service_verified.as_ref() == Some(&context),
        ))
    }
    pub(super) fn setup_help(&self) -> Option<&'static str> {
        match self.setup_step()? {
            SetupStep::Initialize => Some(tr("setup.initialize_help")),
            SetupStep::RegisterService => Some(tr("setup.service_help")),
            SetupStep::Ready | SetupStep::Inspect => None,
        }
    }
    pub(super) fn ask_setup(&mut self, operation: SetupOperation) {
        self.confirm = Some(Action {
            label: operation.title().into(),
            args: Vec::new(),
            secret: None,
            install: None,
            setup: Some(operation),
        });
    }
    pub(super) fn setup_operation(&mut self, operation: SetupOperation) {
        if self.busy {
            return;
        }
        let Some(expected) = self.setup_context() else {
            self.refresh();
            return;
        };
        let required = match operation {
            SetupOperation::Initialize => SetupStep::Initialize,
            SetupOperation::RegisterService => SetupStep::RegisterService,
        };
        if self.setup_step() != Some(required) {
            self.refresh();
            return;
        }
        self.busy = true;
        self.failed = false;
        self.notice = i18n::message("status.working", &[("action", operation.title())]);
        let backend = Backend {
            binary: expected.binary.clone(),
            config: Some(expected.config.clone()),
        };
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<Json, String> {
                let installation = core_install::status()?;
                let fresh = backend.run(&words(&["status"]), None)?;
                if context(&installation, &backend.binary, &fresh).as_ref() != Some(&expected)
                    || setup_step(&fresh, false) != required
                {
                    return Err(tr("setup.changed").into());
                }
                let args = match operation {
                    SetupOperation::Initialize => words(&["init"]),
                    SetupOperation::RegisterService => words(&["service", "install"]),
                };
                let mut result = backend.run(&args, None)?;
                if result.get("ok") != &Json::Bool(true) {
                    return Err(tr("error.invalid_response").into());
                }
                let correct = match operation {
                    SetupOperation::Initialize => result.get("path").text() == expected.config,
                    SetupOperation::RegisterService => {
                        result.get("service_started") == &Json::Bool(false)
                            && std::path::Path::new(result.get("unit_path").text()).is_absolute()
                    }
                };
                if !correct {
                    return Err(tr("error.invalid_response").into());
                }
                if let Json::Object(fields) = &mut result {
                    fields.insert("setup_binary".into(), Json::String(expected.binary));
                    fields.insert("setup_config".into(), Json::String(expected.config));
                }
                Ok(result)
            })();
            let _ = sender.send(Reply {
                label: operation.label().into(),
                result,
                refresh: true,
                starts_gateway: false,
            });
        });
    }
    pub(super) fn accept_setup(&mut self, label: &str, result: &Json) {
        if label == SetupOperation::RegisterService.label()
            && result.get("ok") == &Json::Bool(true)
            && result.get("service_started") == &Json::Bool(false)
            && let Some(context) = setup_result_context(result)
            && self.setup_context().as_ref() == Some(&context)
        {
            self.setup_service_verified = Some(context);
        }
    }
    pub(super) fn trigger_setup_step(&mut self, step: SetupStep) {
        match step {
            SetupStep::Initialize => self.ask_setup(SetupOperation::Initialize),
            SetupStep::RegisterService => self.ask_setup(SetupOperation::RegisterService),
            SetupStep::Ready => self.ask(
                tr("action.start_service_confirm"),
                words(&["service", "start"]),
            ),
            SetupStep::Inspect => self.refresh(),
        }
    }
    pub(super) fn spawn_refresh(&self) {
        let backend = self.backend.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let mut state = backend.refresh();
            if let Json::Object(object) = &mut state {
                match core_install::status() {
                    Ok(info) if info != Json::Null => {
                        object.insert("installation".into(), info);
                    }
                    Err(error) => {
                        object.insert("installation_error".into(), Json::String(error));
                    }
                    _ => {}
                }
            }
            let _ = sender.send(Reply {
                label: "refresh".into(),
                result: Ok(state),
                refresh: false,
                starts_gateway: false,
            });
        });
    }
    pub(super) fn core_operation(&mut self, operation: Operation) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.failed = false;
        self.notice = tr(match operation {
            Operation::Prepare => "install.preparing",
            Operation::Status => "install.checking",
            _ => "install.working",
        })
        .into();
        let binary = self.backend.binary.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = operation.perform(&binary);
            let _ = sender.send(Reply {
                label: format!("Core installer {}", operation.id()),
                result,
                refresh: false,
                starts_gateway: false,
            });
        });
    }
    pub(super) fn ask_install(&mut self, operation: Operation) {
        let label = tr(match operation {
            Operation::Rollback => "install.restore",
            Operation::Resume => "install.finish",
            _ => "install.apply",
        });
        self.confirm = Some(Action {
            label: label.into(),
            args: Vec::new(),
            secret: None,
            install: Some(operation),
            setup: None,
        });
    }
    pub(super) fn accept_installation(&mut self, info: Json, ask: bool, select_binary: bool) {
        if info == Json::Null {
            return;
        }
        // A successful status RPC is not evidence of a committed installation.
        let complete = committed(&info);
        self.failed =
            !info.get("ok").boolean() || matches!(phase(&info), "failed" | "needs_attention");
        self.notice = if self.failed {
            tr("install.recovery_help").into()
        } else {
            notice(&info).unwrap_or(tr("install.checking")).into()
        };
        if complete && select_binary {
            let path = info.get("binary_path").text();
            if !path.is_empty()
                && (path != self.backend.binary
                    || self.preferences.core_binary.as_deref() != Some(path))
            {
                self.backend.binary = path.into();
                self.preferences.core_binary = Some(path.into());
                if let Err(error) = self.preferences.save() {
                    self.failed = true;
                    self.notice = i18n::message("install.path_save_failed", &[("error", &error)]);
                }
            }
        }
        self.core_installation = info;
        if ask && phase(&self.core_installation) == "planned" && !self.failed {
            self.ask_install(Operation::Apply);
        }
    }
    pub(super) fn installation_notice(&self) -> Option<&'static str> {
        // Completion belongs to the Updates card. Only pending work persists
        // across unrelated refreshes so it cannot turn into an "updated" toast.
        if waiting(&self.core_installation) {
            notice(&self.core_installation)
        } else {
            None
        }
    }
    pub(super) fn missing_gateway(&self) -> bool {
        core_install::resolve_binary(&self.backend.binary).is_none()
    }
    pub(super) fn start_core_install(&mut self) {
        self.page = 5;
        self.scroll = 0;
        self.repaint = true;
        match phase(&self.core_installation) {
            "planned" => self.ask_install(Operation::Apply),
            "pending_exit" | "rollback_pending" | "prepared" | "activating" | "old_saved"
            | "installed" => self.ask_install(Operation::Resume),
            "pending_operation" | "preparing" | "needs_attention" => {
                self.core_operation(Operation::Status)
            }
            _ => self.core_operation(Operation::Prepare),
        }
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "an update card carries its existing page layout and data"
    )]
    pub(super) fn gateway_install_card(
        &mut self,
        c: &mut Canvas<'_>,
        x: f32,
        y: f32,
        width: f32,
        title: &str,
        current: &str,
        update: &Json,
        enabled: bool,
    ) {
        card(c, Rect::new(x, y, width, 178.));
        let info = self.core_installation.clone();
        let pending = waiting(&info);
        let recoverable = !info.get("transaction").text().is_empty();
        let missing = self.missing_gateway();
        let managed =
            update.get("managed").boolean() || core_install::package_managed(&self.backend.binary);
        let setup = self.setup_step().filter(|step| {
            *step != SetupStep::Inspect && !self.state.get("status").get("healthy").boolean()
        });
        c.text(title, x + 24., y + 20., 20, true, TEXT);
        c.ellipsis(
            &i18n::message("updates.current", &[("current", current)]),
            x + 24.,
            y + 55.,
            (width - 215.).max(135.),
            14,
            false,
            MUTED,
        );
        let label = if managed {
            tr("action.package_update")
        } else if let Some(step) = setup {
            tr(match step {
                SetupStep::Initialize => "setup.initialize",
                SetupStep::RegisterService => "setup.install_service",
                SetupStep::Ready => "action.start_gateway",
                SetupStep::Inspect => "action.retry",
            })
        } else if pending {
            tr("install.finish")
        } else if phase(&info) == "planned" {
            tr("install.apply")
        } else if missing {
            tr("install.bootstrap")
        } else {
            tr("install.update")
        };
        let button_width = (c.measure(label, 14, false) + 38.).clamp(146., 220.);
        if self.ui.button(
            c,
            850,
            Rect::new(x + width - button_width - 24., y + 31., button_width, 42.),
            label,
            Style::Primary,
            enabled && phase(&info) != "pending_operation",
        ) {
            if managed {
                self.package_update(false);
            } else if let Some(step) = setup {
                self.trigger_setup_step(step);
            } else {
                self.start_core_install();
            }
        }
        let status = if pending {
            tr("install.waiting")
        } else if committed(&info) {
            tr("install.completed")
        } else if matches!(phase(&info), "failed" | "needs_attention") {
            tr("install.recovery_help")
        } else if phase(&info) == "planned" {
            tr("install.plan_ready")
        } else {
            tr("install.full_bundle")
        };
        c.ellipsis(
            self.setup_help().unwrap_or(status),
            x + 24.,
            y + 91.,
            width - 48.,
            14,
            false,
            if pending { TEXT } else { MUTED },
        );
        if info != Json::Null {
            if self.ui.button(
                c,
                860,
                Rect::new(x + 24., y + 127., 96., 32.),
                tr("install.details"),
                Style::Quiet,
                enabled,
            ) {
                self.install_details = true;
            }
            if self.ui.button(
                c,
                861,
                Rect::new(x + 130., y + 127., 126., 32.),
                tr("install.status"),
                Style::Secondary,
                enabled,
            ) {
                self.core_operation(Operation::Status);
            }
            if recoverable
                && !committed(&info)
                && self.ui.button(
                    c,
                    862,
                    Rect::new(x + 266., y + 127., (width - 290.).clamp(100., 154.), 32.),
                    tr("install.restore"),
                    Style::Quiet,
                    enabled,
                )
            {
                self.ask_install(Operation::Rollback);
            }
        } else if !missing
            && self.ui.button(
                c,
                863,
                Rect::new(x + 24., y + 127., 156., 32.),
                tr("action.check_update"),
                Style::Quiet,
                enabled,
            )
        {
            // The existing controller decides which operating-system package
            // manager owns it. Preparing a native install never adopts that path.
            self.action("Check gateway update", words(&["update", "check"]), None);
        }
    }
    pub(super) fn installation_dialog(&mut self, c: &mut Canvas<'_>) {
        c.rect(Rect::new(0., 0., c.width, c.height), 0., 0x00000066);
        let width = (c.width - 48.).min(640.);
        let height = (c.height - 48.).min(438.);
        let r = Rect::new(
            (c.width - width) / 2.,
            (c.height - height) / 2.,
            width,
            height,
        );
        c.rect(r, 12., SURFACE);
        c.text(tr("install.details"), r.x + 24., r.y + 25., 22, true, TEXT);
        let fields = [
            ("install.version", "version"),
            ("install.location", "prefix"),
            ("install.phase", "phase"),
            ("install.source", "source"),
            ("install.journal", "transaction"),
            ("install.verification", "verified_source"),
            ("install.error", "error"),
        ];
        for (index, (label, key)) in fields.iter().enumerate() {
            let y = r.y + 72. + index as f32 * 41.;
            c.text(tr(label), r.x + 24., y, 12, true, MUTED);
            c.ellipsis(
                value(&self.core_installation, key, "—"),
                r.x + 24.,
                y + 17.,
                width - 48.,
                13,
                false,
                TEXT,
            );
        }
        if self.ui.button(
            c,
            9681,
            Rect::new(r.x + width - 140., r.y + height - 56., 116., 36.),
            tr("install.close"),
            Style::Secondary,
            true,
        ) {
            self.install_details = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let mut app = App::unloaded(
            Backend {
                binary: "missing-test-core".into(),
                config: None,
            },
            settings::Preferences::test_defaults(),
        );
        app.busy = false;
        app
    }
    #[test]
    fn pending_and_helper_launch_never_commit_or_change_binary() {
        for data in [
            r#"{"ok":true,"phase":"pending_exit","pending_exit":true,"helper_started":true,"binary_path":"/not/installed"}"#,
            r#"{"ok":true,"phase":"committed","committed":true,"helper_started":true,"binary_path":"/not/installed"}"#,
            r#"{"ok":true,"phase":"pending_operation"}"#,
        ] {
            let mut app = app();
            let info = json::parse(data).unwrap();
            assert!(!committed(&info));
            app.accept_installation(info, false, true);
            assert_eq!(app.backend.binary, "missing-test-core");
            assert!(app.preferences.core_binary.is_none());
            assert!(app.installation_notice().is_some());
        }
    }
    #[test]
    fn saved_plan_is_reviewable_but_never_automatically_applied() {
        let mut app = app();
        let info = json::parse(r#"{"ok":true,"phase":"planned","version":"1.2.3"}"#).unwrap();
        app.accept_installation(info.clone(), false, false);
        assert!(app.confirm.is_none());
        assert!(!app.busy);
        app.accept_installation(info, true, true);
        assert_eq!(
            app.confirm.as_ref().unwrap().install,
            Some(Operation::Apply)
        );
        assert!(!app.busy);
        assert!(app.receiver.try_recv().is_err());
    }
    #[test]
    fn pending_state_survives_other_refresh_without_false_success() {
        let mut app = app();
        app.accept_installation(
            json::parse(r#"{"ok":true,"phase":"pending_exit","pending_exit":true}"#).unwrap(),
            false,
            false,
        );
        app.sender
            .send(Reply {
                label: "refresh".into(),
                result: json::parse(r#"{"status":{"healthy":true}}"#),
                refresh: false,
                starts_gateway: false,
            })
            .unwrap();
        assert!(app.poll());
        assert_eq!(app.notice, tr("install.waiting_help"));
        assert!(!committed(&app.core_installation));
    }
    #[test]
    fn passive_installer_refresh_preserves_explicit_binary_selection() {
        let mut app = app();
        app.accept_installation(json::parse(r#"{"ok":true,"phase":"committed","committed":true,"binary_path":"/other/private/bin/zcalld"}"#).unwrap(), false, false);
        assert_eq!(app.backend.binary, "missing-test-core");
        assert!(app.preferences.core_binary.is_none());
        assert!(!committed(
            &json::parse(r#"{"ok":false,"phase":"committed","committed":true}"#).unwrap()
        ));
    }
    fn setup_app(status: &str) -> App {
        let mut app = app();
        let prefix = std::env::temp_dir().join("zcall-gui-unit-private-prefix");
        let binary = prefix
            .join("bin")
            .join(if cfg!(windows) {
                "zcalld.exe"
            } else {
                "zcalld"
            })
            .to_string_lossy()
            .into_owned();
        app.backend.binary = binary.clone();
        app.core_installation = Json::object([
            ("ok".into(), Json::Bool(true)),
            ("phase".into(), Json::String("committed".into())),
            ("committed".into(), Json::Bool(true)),
            (
                "prefix".into(),
                Json::String(prefix.to_string_lossy().into_owned()),
            ),
            ("binary_path".into(), Json::String(binary)),
        ]);
        let mut status = json::parse(status).unwrap();
        if let Json::Object(fields) = &mut status {
            fields.insert(
                "configuration_path".into(),
                Json::String(
                    std::env::temp_dir()
                        .join("zcall-gui-unit-config.conf")
                        .to_string_lossy()
                        .into_owned(),
                ),
            );
        }
        app.state = Json::object([("status".into(), status)]);
        app
    }
    #[test]
    fn bootstrap_only_initializes_an_explicitly_missing_configuration() {
        let mut app =
            setup_app(r#"{"configured":false,"configuration_exists":false,"healthy":false}"#);
        assert_eq!(app.setup_step(), Some(SetupStep::Initialize));
        assert!(app.receiver.try_recv().is_err());
        app.ask_setup(SetupOperation::Initialize);
        assert_eq!(
            app.confirm.as_ref().unwrap().setup,
            Some(SetupOperation::Initialize)
        );
        assert!(!app.busy);
        assert!(app.receiver.try_recv().is_err());
        for value in [Json::Null, Json::Bool(true), Json::String("false".into())] {
            if let Json::Object(root) = &mut app.state
                && let Some(Json::Object(status)) = root.get_mut("status")
            {
                status.insert("configuration_exists".into(), value);
            }
            assert_eq!(app.setup_step(), Some(SetupStep::Inspect));
        }
        app.state = Json::object([(
            "status".into(),
            json::parse(r#"{"healthy":false}"#).unwrap(),
        )]);
        assert_eq!(app.setup_step(), Some(SetupStep::Inspect));
        app.backend.binary = "other-core".into();
        assert_eq!(app.setup_step(), None);
    }
    #[test]
    fn existing_service_does_not_skip_explicit_ownership_verification() {
        let mut app = setup_app(
            r#"{"configured":true,"configuration_exists":true,"service_install_supported":true,"service_installed":true,"healthy":false}"#,
        );
        assert_eq!(app.setup_step(), Some(SetupStep::RegisterService));
        let context = app.setup_context().unwrap();
        let mut result = Json::object([
            ("ok".into(), Json::Bool(true)),
            ("service_started".into(), Json::Bool(false)),
            ("setup_binary".into(), Json::String(context.binary)),
            ("setup_config".into(), Json::String(context.config)),
        ]);
        app.accept_setup("other action", &result);
        assert_eq!(app.setup_step(), Some(SetupStep::RegisterService));
        app.accept_setup("Bootstrap service install", &result);
        assert_eq!(app.setup_step(), Some(SetupStep::Ready));
        if let Json::Object(fields) = &mut result {
            fields.insert("setup_config".into(), Json::String("different".into()));
        }
        app.setup_service_verified = None;
        app.accept_setup("Bootstrap service install", &result);
        assert_eq!(app.setup_step(), Some(SetupStep::RegisterService));
    }
}
