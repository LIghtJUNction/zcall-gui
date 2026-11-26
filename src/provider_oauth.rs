use crate::{App, Backend, Json, Reply, json, tr, words};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const LMM_DOMAIN: &str = "api.lmm.best";
pub const LMM_PLUGIN: &str = "lmm-oauth";
pub const LMM_SOURCE: &str = "github:TokenNotIncluded/opencode-lmm-auth";
pub const LMM_REVISION: &str = "c32312c8d1e03c28e1204d33e7e94d1b312ac832";
pub const LMM_REPO: &str = "https://github.com/TokenNotIncluded/opencode-lmm-auth.git";

fn issuer(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let host = trimmed
        .strip_prefix("https://")
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    if !host.contains('.')
        || host.len() > 253
        || !host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err(tr("providers.oauth_invalid_domain").into());
    }
    Ok(format!("https://{}", host.to_ascii_lowercase()))
}

pub(super) fn endpoint_matches(query: &str, candidate: &str) -> bool {
    let candidate = candidate.to_ascii_lowercase();
    query.to_ascii_lowercase().split_whitespace().all(|token| {
        if candidate.contains(token) {
            return true;
        }
        let mut chars = candidate.chars();
        token.chars().all(|wanted| chars.any(|c| c == wanted))
    })
}

struct Session {
    child: Child,
    replies: mpsc::Receiver<Result<Json, String>>,
    next: u64,
}

impl Session {
    fn start(backend: &Backend) -> Result<Self, String> {
        let mut command = Command::new(&backend.binary);
        command.args(["accounts", "session", "--json"]);
        if let Some(config) = &backend.config {
            command.arg("--config").arg(config);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| tr("providers.oauth_unavailable"))?;
        let stdout = child.stdout.take().expect("piped account stdout");
        let (sender, replies) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match stdout
                    .by_ref()
                    .take(2 * 1024 * 1024 + 1)
                    .read_line(&mut line)
                {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let reply = json::parse(&line)
                            .map_err(|_| tr("providers.oauth_unavailable").into());
                        if sender.send(reply).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self {
            child,
            replies,
            next: 0,
        })
    }

    fn call(&mut self, method: &str, params: Json) -> Result<Json, String> {
        self.next += 1;
        let request = Json::object([
            ("id".into(), Json::Number(self.next.to_string())),
            ("method".into(), Json::String(method.into())),
            ("params".into(), params),
        ]);
        let stdin = self.child.stdin.as_mut().expect("piped account stdin");
        writeln!(stdin, "{}", request.stringify())
            .map_err(|_| tr("providers.oauth_unavailable"))?;
        let timeout = if method == "callback" { 185 } else { 30 };
        let response = self
            .replies
            .recv_timeout(Duration::from_secs(timeout))
            .map_err(|_| tr("providers.oauth_unavailable"))??;
        if response.get("id").text() != self.next.to_string()
            || response.get("result") == &Json::Null
        {
            return Err(tr("providers.oauth_unavailable").into());
        }
        Ok(response.get("result").clone())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.child.stdin.take();
        // The same helper owns both the browser callback and credential save.
        // Closing stdin lets it clean up its host even if the UI stops waiting.
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn open_browser(url: &str) -> Result<(), String> {
    let Some(rest) = url.strip_prefix("https://") else {
        return Err(tr("providers.oauth_bad_link").into());
    };
    if rest.is_empty()
        || rest
            .split(['/', '?', '#'])
            .next()
            .is_none_or(|host| host.is_empty() || host.contains('@'))
        || url
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return Err(tr("providers.oauth_bad_link").into());
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer.exe"
    } else {
        "xdg-open"
    };
    let mut child = Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| tr("providers.oauth_bad_link"))?;
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn configure_lmm(backend: &Backend, domain: &str, providers: &[Json]) -> Result<(), String> {
    let plugins = backend.run(&words(&["plugins", "list"]), None)?;
    let plugin = plugins
        .get("plugins")
        .array()
        .iter()
        .find(|item| {
            item.get("name").text() == LMM_PLUGIN
                && item.get("source").text() == LMM_REPO
                && item.get("revision").text() == LMM_REVISION
                && item.get("enabled").boolean()
        })
        .ok_or_else(|| tr("providers.oauth_unavailable").to_owned())?;
    let accounts =
        if plugin.get("configured").boolean() || domain != format!("https://{LMM_DOMAIN}") {
            backend.run(&words(&["accounts", "list"]), None)?
        } else {
            Json::Null
        };
    if accounts
        .get("accounts")
        .array()
        .iter()
        .any(|item| item.get("provider").text() == "lmm")
    {
        if providers.iter().any(|item| {
            item.get("plugin").text() == "lmm"
                && item.get("base_url").text() == format!("{domain}/v1")
        }) {
            return Ok(());
        }
        return Err(tr("providers.oauth_issuer_in_use").into());
    }
    if domain == format!("https://{LMM_DOMAIN}") && !plugin.get("configured").boolean() {
        return Ok(());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("zcall-issuer-{}-{nonce}.json", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let contents = Json::object([("issuer".into(), Json::String(domain.into()))]).stringify();
    let result = file.write_all(contents.as_bytes());
    drop(file);
    let result = result.map_err(|e| e.to_string()).and_then(|()| {
        backend.run(
            &words(&[
                "plugins",
                "config",
                LMM_PLUGIN,
                "--file",
                &path.to_string_lossy(),
            ]),
            None,
        )
    });
    let _ = std::fs::remove_file(path);
    result.map(|_| ())
}

struct Login {
    name: String,
    domain: String,
    plugin: String,
    protocol: &'static str,
}

fn available_name(requested: &str, providers: &[Json]) -> String {
    let occupied = |name: &str| providers.iter().any(|item| item.get("name").text() == name);
    if !occupied(requested) {
        return requested.into();
    }
    for index in 2.. {
        let suffix = format!("-{index}");
        let prefix = &requested[..requested.len().min(128 - suffix.len())];
        let name = format!("{prefix}{suffix}");
        if !occupied(&name) {
            return name;
        }
    }
    unreachable!("finite provider list");
}

fn login(
    backend: &Backend,
    login: Login,
    sender: &mpsc::Sender<Reply>,
    code: mpsc::Receiver<String>,
) -> Result<Json, String> {
    let providers = backend.run(&words(&["providers", "list"]), None)?;
    if !providers.get("oauth_binding").boolean() {
        return Err(tr("providers.oauth_update_gateway").into());
    }
    let providers = providers.get("providers").array();
    let name = available_name(&login.name, providers);
    let base = format!("{}/v1", login.domain);
    if login.plugin == "lmm" {
        configure_lmm(backend, &login.domain, providers)?;
    }
    let mut session = Session::start(backend)?;
    let options = session.call("providers", Json::object([]))?;
    let provider = options
        .array()
        .iter()
        .find(|row| {
            row.get("id").text() == login.plugin
                && (login.plugin != "lmm" || row.get("spec").text() == LMM_REPO)
        })
        .ok_or_else(|| tr("providers.oauth_missing_plugin").to_owned())?;
    let method = provider
        .get("methods")
        .array()
        .iter()
        .position(|method| method.get("type").text() == "oauth")
        .ok_or_else(|| tr("providers.oauth_missing_plugin").to_owned())?;
    let started = session.call(
        "authorize",
        Json::object([
            ("provider".into(), Json::String(login.plugin.clone())),
            ("method".into(), Json::Number(method.to_string())),
        ]),
    )?;
    if !started.get("ok").boolean() || started.get("session").text().is_empty() {
        return Err(tr("providers.oauth_unavailable").into());
    }
    let url = started.get("url").text();
    if !started.get("opened").boolean() && !url.is_empty() {
        open_browser(url)?;
    }
    let mut params = vec![("session".into(), started.get("session").clone())];
    if started.get("method").text() == "code" {
        let _ = sender.send(Reply {
            label: "OAuth code".into(),
            result: Ok(Json::String(started.get("instructions").text().to_owned())),
            refresh: false,
            starts_gateway: false,
        });
        let entered = code
            .recv_timeout(Duration::from_secs(180))
            .map_err(|_| tr("providers.oauth_cancelled"))?;
        params.push(("code".into(), Json::String(entered)));
    }
    let finished = session.call("callback", Json::object(params))?;
    let provider = finished.get("provider").text();
    let account = finished.get("account").text();
    if !finished.get("ok").boolean() || provider.is_empty() || account.is_empty() {
        return Err(tr("providers.oauth_cancelled").into());
    }
    backend.run(
        &words(&[
            "providers",
            "add",
            &name,
            "--base-url",
            &base,
            "--protocol",
            login.protocol,
            "--plugin",
            provider,
            "--plugin-account",
            account,
        ]),
        None,
    )
}

impl App {
    pub(super) fn start_oauth(&mut self) {
        let result = issuer(&self.provider.url);
        let domain = match result {
            Ok(domain) => domain,
            Err(error) => {
                self.notice = error;
                self.failed = true;
                return;
            }
        };
        let plugin = self.provider.plugin_id.trim().to_owned();
        let name = if self.provider.name.trim().is_empty() {
            plugin.clone()
        } else {
            self.provider.name.trim().to_owned()
        };
        let login_options = Login {
            name,
            domain,
            plugin,
            protocol: crate::protocols()[self.provider.protocol],
        };
        let (input, code) = mpsc::channel();
        self.oauth_input = Some(input);
        self.oauth_code.clear();
        self.oauth_instructions.clear();
        self.busy = true;
        self.failed = false;
        self.repaint = true;
        let backend = self.backend.clone();
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = login(&backend, login_options, &sender, code);
            let _ = sender.send(Reply {
                label: "OAuth provider".into(),
                refresh: result.is_ok(),
                result,
                starts_gateway: false,
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issuer_accepts_domains_and_rejects_redirect_like_inputs() {
        assert_eq!(issuer(" API.LMM.BEST ").unwrap(), "https://api.lmm.best");
        assert_eq!(
            issuer("https://api.lmm.best/").unwrap(),
            "https://api.lmm.best"
        );
        for url in [
            "http://api.lmm.best",
            "example.com/path",
            "example.com@evil.test",
            "host",
            "host.test?key=value",
        ] {
            assert!(issuer(url).is_err(), "{url}");
        }
    }

    #[test]
    fn duplicate_names_leave_existing_providers_in_place() {
        let providers =
            ["lmm", "lmm-2"].map(|name| Json::object([("name".into(), Json::String(name.into()))]));
        assert_eq!(available_name("lmm", &providers), "lmm-3");
        assert_eq!(available_name("other", &providers), "other");
    }

    #[test]
    fn endpoint_search_matches_names_addresses_and_nonconsecutive_letters() {
        let row = "DeepSeek https://api.deepseek.com/v1";
        assert!(endpoint_matches("dip seek", "Diverse IP Seek"));
        assert!(endpoint_matches("dpsk", row));
        assert!(endpoint_matches("DEEP /v1", row));
        assert!(!endpoint_matches("ollama", row));
    }
}
