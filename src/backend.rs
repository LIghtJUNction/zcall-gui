use crate::json::{self, Json};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Backend {
    pub binary: String,
    pub config: Option<String>,
}
pub struct Reply {
    pub label: String,
    pub result: Result<Json, String>,
    pub refresh: bool,
    pub starts_gateway: bool,
}
impl Backend {
    pub fn run(&self, args: &[String], mut secret: Option<Vec<u8>>) -> Result<Json, String> {
        let mut command = Command::new(&self.binary);
        command.args(args).arg("--json");
        if let Some(config) = &self.config {
            command.arg("--config").arg(config);
        }
        command
            .stdin(if secret.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|e| {
            format!("Cannot run zcalld: {e}. Install zcalld or pass --zcalld PATH.")
        })?;
        if let Some(bytes) = secret.as_mut() {
            let result = child
                .stdin
                .take()
                .ok_or("zcalld stdin unavailable")
                .and_then(|mut stdin| {
                    stdin
                        .write_all(bytes)
                        .map_err(|_| "Cannot send credential to zcalld")
                });
            bytes.fill(0);
            if let Err(error) = result {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
        let stdout = child.stdout.take().ok_or("zcalld stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("zcalld stderr unavailable")?;
        let read = |stream: Box<dyn Read + Send>| {
            let mut bytes = Vec::new();
            stream
                .take(2 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        };
        let out_thread = thread::spawn(move || read(Box::new(stdout)));
        let err_thread = thread::spawn(move || read(Box::new(stderr)));
        let deadline = Instant::now() + Duration::from_secs(180);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("zcalld operation timed out or could not be monitored".into());
                }
            }
        };
        let stdout = out_thread
            .join()
            .map_err(|_| "zcalld stdout reader stopped")?
            .map_err(|e| e.to_string())?;
        let stderr = err_thread
            .join()
            .map_err(|_| "zcalld stderr reader stopped")?
            .map_err(|e| e.to_string())?;
        if stdout.len() > 2 * 1024 * 1024 || stderr.len() > 2 * 1024 * 1024 {
            return Err("zcalld output exceeded the 2 MiB bound".into());
        }
        let output = String::from_utf8(stdout).map_err(|_| "zcalld returned non-UTF-8 output")?;
        let parsed = json::parse(&output);
        if !status.success() {
            let detail = parsed
                .as_ref()
                .ok()
                .map(|j| {
                    let e = j.get("error");
                    if e.text().is_empty() {
                        e.get("message").text()
                    } else {
                        e.text()
                    }
                })
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| String::from_utf8_lossy(&stderr).chars().take(500).collect());
            return Err(if detail.trim().is_empty() {
                format!("zcalld exited with {status}")
            } else {
                detail
            });
        }
        parsed
    }
    pub fn refresh(&self) -> Json {
        let groups = [
            ("status", vec!["status"]),
            ("providers", vec!["providers", "list"]),
            ("agents", vec!["agents", "list"]),
            ("plugins", vec!["plugins", "list"]),
            ("privacy", vec!["privacy", "show"]),
            ("models", vec!["models"]),
        ];
        Json::object(groups.into_iter().map(|(name, args)| {
            let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
            let value = self
                .run(&args, None)
                .unwrap_or_else(|error| Json::object([("error".into(), Json::String(error))]));
            (name.into(), value)
        }))
    }
    pub fn spawn(
        &self,
        sender: mpsc::Sender<Reply>,
        label: String,
        args: Vec<String>,
        secret: Option<Vec<u8>>,
    ) {
        let backend = self.clone();
        let starts_gateway = args.first().is_some_and(|arg| arg == "service")
            && args
                .get(1)
                .is_some_and(|arg| matches!(arg.as_str(), "start" | "restart"));
        thread::spawn(move || {
            let result = backend.run(&args, secret);
            let _ = sender.send(Reply {
                label,
                result,
                refresh: true,
                starts_gateway,
            });
        });
    }
    pub fn spawn_refresh(&self, sender: mpsc::Sender<Reply>) {
        let backend = self.clone();
        thread::spawn(move || {
            let _ = sender.send(Reply {
                label: "refresh".into(),
                result: Ok(backend.refresh()),
                refresh: false,
                starts_gateway: false,
            });
        });
    }
}
