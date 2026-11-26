use std::path::Path;

pub fn apply(binary: &Path, packages: &[&str]) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let terminal = std::env::var_os("TERMINAL");
        linux::Plan::discover(binary, packages, &path, terminal.as_deref())?
            .run(std::time::Duration::from_secs(7200))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (binary, packages);
        Err(crate::i18n::tr("updates.no_pacman").into())
    }
}

pub fn gateway(binary: &Path) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let terminal = std::env::var_os("TERMINAL");
        linux::Plan::gateway(binary, &path, terminal.as_deref())?
            .run(std::time::Duration::from_secs(7200))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = binary;
        Err(crate::i18n::tr("updates.no_terminal").into())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::i18n::{message, tr};
    use std::{
        ffi::{OsStr, OsString},
        fs,
        os::unix::fs::{DirBuilderExt, PermissionsExt},
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    // Every variable enters the fixed script as an argv element, never as shell source.
    // The shell remains alive while the helper runs, including detached terminal launchers.
    const WRAPPER: &str = r#"result=$1
shift
printf '%s\n' "$$" > "$result.started"
trap 'code=$?; printf "%s\n" "$code" > "$result"; exit "$code"' 0
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
"$@"
"#;

    fn executable(path: &Path) -> Option<PathBuf> {
        path.metadata()
            .ok()
            .filter(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .and_then(|_| path.canonicalize().ok())
    }

    fn find(name: &OsStr, path: &OsStr) -> Option<PathBuf> {
        let name = Path::new(name);
        if name.components().count() > 1 {
            return executable(name);
        }
        std::env::split_paths(path).find_map(|dir| executable(&dir.join(name)))
    }

    fn terminal_args(name: &str) -> Option<&'static [&'static str]> {
        match name {
            "xdg-terminal-exec" | "kitty" | "foot" => Some(&["--"]),
            "alacritty" | "konsole" | "xterm" => Some(&["-e"]),
            "wezterm" => Some(&["start", "--"]),
            "gnome-terminal" => Some(&["--wait", "--"]),
            "xfce4-terminal" => Some(&["--disable-server", "--execute"]),
            _ => None,
        }
    }

    fn terminal(path: &OsStr, preferred: Option<&OsStr>) -> Option<(PathBuf, Vec<OsString>)> {
        let names = preferred.into_iter().chain(
            [
                "xdg-terminal-exec",
                "kitty",
                "foot",
                "alacritty",
                "wezterm",
                "konsole",
                "gnome-terminal",
                "xfce4-terminal",
                "xterm",
            ]
            .into_iter()
            .map(OsStr::new),
        );
        for name in names {
            let Some(base) = Path::new(name).file_name().and_then(OsStr::to_str) else {
                continue;
            };
            let Some(args) = terminal_args(base) else {
                continue;
            };
            if let Some(binary) = find(name, path) {
                return Some((binary, args.iter().map(OsString::from).collect()));
            }
        }
        None
    }

    pub(super) struct Plan {
        executable: PathBuf,
        command: PathBuf,
        args: Vec<OsString>,
        terminal: PathBuf,
        terminal_args: Vec<OsString>,
    }

    impl Plan {
        pub(super) fn discover(
            binary: &Path,
            packages: &[&str],
            path: &OsStr,
            preferred: Option<&OsStr>,
        ) -> Result<Self, String> {
            let executable = find(binary.as_os_str(), path).ok_or(tr("updates.not_owned"))?;
            let pacman = find(OsStr::new("pacman"), path).ok_or(tr("updates.no_pacman"))?;
            let output = Command::new(pacman)
                .arg("-Qqo")
                .arg(&executable)
                .output()
                .map_err(|e| message("updates.start_failed", &[("error", &e.to_string())]))?;
            let package = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !output.status.success() || !packages.contains(&package.as_str()) {
                return Err(tr("updates.not_owned").into());
            }
            let helper = ["paru", "yay"]
                .into_iter()
                .find_map(|name| find(OsStr::new(name), path))
                .ok_or(tr("updates.no_helper"))?;
            let (terminal, terminal_args) =
                terminal(path, preferred).ok_or(tr("updates.no_terminal"))?;
            Ok(Self {
                executable,
                command: helper,
                args: vec!["-S".into(), "--needed".into(), "--".into(), package.into()],
                terminal,
                terminal_args,
            })
        }

        pub(super) fn gateway(
            binary: &Path,
            path: &OsStr,
            preferred: Option<&OsStr>,
        ) -> Result<Self, String> {
            let executable = find(binary.as_os_str(), path).ok_or(tr("updates.not_owned"))?;
            let (terminal, terminal_args) =
                terminal(path, preferred).ok_or(tr("updates.no_terminal"))?;
            Ok(Self {
                command: executable.clone(),
                executable,
                args: vec!["update".into(), "apply".into()],
                terminal,
                terminal_args,
            })
        }

        pub(super) fn run(&self, timeout: Duration) -> Result<String, String> {
            let temp = PrivateDir::new()
                .map_err(|e| message("updates.start_failed", &[("error", &e.to_string())]))?;
            let result = temp.0.join("result");
            let started = temp.0.join("result.started");
            let mut child = Command::new(&self.terminal)
                .args(&self.terminal_args)
                .args(["/bin/sh", "-c", WRAPPER, "zcall-package-update"])
                .arg(&result)
                .arg(&self.command)
                .args(&self.args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| message("updates.start_failed", &[("error", &e.to_string())]))?;
            let begin = Instant::now();
            let outcome = loop {
                if let Ok(text) = fs::read_to_string(&result)
                    && let Ok(code) = text.trim().parse::<i32>()
                {
                    break if code == 0 {
                        Ok(())
                    } else {
                        Err(message(
                            "updates.package_failed",
                            &[("code", &code.to_string())],
                        ))
                    };
                }
                let status = child.try_wait().map_err(|e| e.to_string())?;
                if let Ok(text) = fs::read_to_string(&started)
                    && let Ok(pid) = text.trim().parse::<u32>()
                    && !Path::new("/proc").join(pid.to_string()).exists()
                {
                    // A result may have arrived between the first read and process exit.
                    if result.exists() {
                        continue;
                    }
                    break Err(tr("updates.terminal_closed").into());
                }
                if !started.exists() {
                    if status.is_some_and(|s| !s.success())
                        || begin.elapsed() > Duration::from_secs(30)
                    {
                        break Err(tr("updates.terminal_closed").into());
                    }
                } else if begin.elapsed() > timeout {
                    // Never kill a package transaction. The terminal remains under user control.
                    break Err(tr("updates.package_timeout").into());
                }
                thread::sleep(Duration::from_millis(200));
            };
            if child.try_wait().ok().flatten().is_none() {
                // Reap a terminal client after it exits without blocking the application.
                thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            outcome?;
            let version = Command::new(&self.executable)
                .arg("--version")
                .output()
                .map_err(|e| message("updates.start_failed", &[("error", &e.to_string())]))?;
            if !version.status.success() {
                return Err(tr("updates.version_failed").into());
            }
            let version = String::from_utf8_lossy(&version.stdout).trim().to_owned();
            if version.is_empty() || version.len() > 120 {
                return Err(tr("updates.version_failed").into());
            }
            Ok(version)
        }
    }

    struct PrivateDir(PathBuf);
    impl PrivateDir {
        fn new() -> std::io::Result<Self> {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "zcall-package-update-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&path)?;
            Ok(Self(path))
        }
    }
    impl Drop for PrivateDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // Unique paths do not isolate the process file-descriptor table. A
        // concurrent fork can inherit another fixture's writable script before
        // exec closes O_CLOEXEC descriptors, causing ETXTBSY after its parent
        // writer is already closed. Serialize only these process fixtures.
        static PROCESS_FIXTURES: std::sync::Mutex<()> = std::sync::Mutex::new(());

        fn script(root: &Path, name: &str, body: &str) -> PathBuf {
            let path = root.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        }

        fn fixture(owner: &str, code: i32, helper: &str) -> (PrivateDir, Plan) {
            fixture_with_terminal(
                owner,
                code,
                helper,
                "printf '%s\\n' \"$@\" > \"$0.args\"\n[ \"$1\" = '--' ] && shift\nexec \"$@\"",
            )
        }

        fn fixture_with_terminal(
            owner: &str,
            code: i32,
            helper: &str,
            terminal: &str,
        ) -> (PrivateDir, Plan) {
            let root = PrivateDir::new().unwrap();
            script(&root.0, "zcall-gui", "printf 'zcall-gui 0.1.0\\n'");
            script(&root.0, "pacman", &format!("printf '{owner}\\n'"));
            script(
                &root.0,
                helper,
                &format!("printf '%s\\n' \"$@\" > \"$0.args\"\nexit {code}"),
            );
            script(&root.0, "xdg-terminal-exec", terminal);
            let plan = Plan::discover(
                Path::new("zcall-gui"),
                &["zcall-gui", "zcall-gui-bin"],
                root.0.as_os_str(),
                None,
            )
            .unwrap();
            (root, plan)
        }

        #[test]
        fn source_and_binary_packages_use_targeted_upgrade() {
            let _fixture = PROCESS_FIXTURES.lock().unwrap();
            for (owner, helper) in [("zcall-gui", "paru"), ("zcall-gui-bin", "yay")] {
                let (root, plan) = fixture(owner, 0, helper);
                assert_eq!(plan.run(Duration::from_secs(5)).unwrap(), "zcall-gui 0.1.0");
                assert_eq!(
                    fs::read_to_string(root.0.join(format!("{helper}.args"))).unwrap(),
                    format!("-S\n--needed\n--\n{owner}\n")
                );
                let args = fs::read_to_string(root.0.join("xdg-terminal-exec.args")).unwrap();
                let result = args.lines().find(|s| s.ends_with("/result")).unwrap();
                assert!(!Path::new(result).parent().unwrap().exists());
            }
        }

        #[test]
        fn gateway_terminal_runs_the_selected_executable_and_update_subcommand() {
            let _fixture = PROCESS_FIXTURES.lock().unwrap();
            let (root, _) = fixture("zcall-gui", 0, "paru");
            script(
                &root.0,
                "zcalld",
                r#"if [ "$1" = '--version' ]; then
    printf 'zcalld 0.4.4\n'
else
    printf '%s\n' "$@" > "$(dirname "$0")/gateway.args"
fi"#,
            );
            let plan = Plan::gateway(Path::new("zcalld"), root.0.as_os_str(), None).unwrap();
            assert_eq!(plan.run(Duration::from_secs(5)).unwrap(), "zcalld 0.4.4");
            assert_eq!(
                fs::read_to_string(root.0.join("gateway.args")).unwrap(),
                "update\napply\n"
            );
            assert!(!root.0.join("paru.args").exists());
        }

        #[test]
        fn helper_failure_is_not_reported_as_success() {
            let _fixture = PROCESS_FIXTURES.lock().unwrap();
            let (_root, plan) = fixture("zcall-gui-bin", 17, "paru");
            assert!(plan.run(Duration::from_secs(5)).unwrap_err().contains("17"));
        }

        #[test]
        fn unrecognized_owner_is_rejected_before_launch() {
            let _fixture = PROCESS_FIXTURES.lock().unwrap();
            let (root, _) = fixture("zcall-gui", 0, "paru");
            script(&root.0, "pacman", "printf 'unrelated-package\\n'");
            assert!(
                Plan::discover(
                    Path::new("zcall-gui"),
                    &["zcall-gui", "zcall-gui-bin"],
                    root.0.as_os_str(),
                    None,
                )
                .is_err()
            );
            assert!(!root.0.join("paru.args").exists());
        }

        #[test]
        fn detached_terminal_result_is_observed() {
            let _fixture = PROCESS_FIXTURES.lock().unwrap();
            let (_root, plan) = fixture_with_terminal(
                "zcall-gui-bin",
                0,
                "paru",
                "[ \"$1\" = '--' ] && shift\n\"$@\" &\nexit 0",
            );
            assert_eq!(plan.run(Duration::from_secs(5)).unwrap(), "zcall-gui 0.1.0");
        }
    }
}
