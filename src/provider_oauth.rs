use crate::{App, Backend, Json, Reply, json, tr, words};
use std::{
    collections::BTreeMap,
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

// These two OpenCode adapters expose OAuth methods without a catalog API base.
const SUBSCRIPTION_PRESETS: [(&str, &str, &str, &str, &str, usize); 2] = [
    (
        "ChatGPT / OpenAI Codex",
        "openai",
        "opencode-openai-codex-auth",
        "4.4.0",
        "https://chatgpt.com/backend-api/codex",
        1,
    ),
    (
        "Kimi Code / Moonshot",
        "kimi-code",
        "opencode-kimi-subscription",
        "0.1.0",
        "https://api.kimi.com/coding/v1",
        0,
    ),
];

pub(super) fn subscription_listings() -> impl Iterator<Item = Json> {
    SUBSCRIPTION_PRESETS
        .into_iter()
        .map(|(name, provider, package, version, _, _)| {
            Json::object([
                ("package".into(), Json::String(package.into())),
                ("name".into(), Json::String(name.into())),
                (
                    "source".into(),
                    Json::String(format!("npm:{package}@{version}")),
                ),
                (
                    "providers".into(),
                    Json::Array(vec![Json::String(provider.into())]),
                ),
            ])
        })
}

pub(super) fn market_priority(package: &str) -> usize {
    [
        "opencode-openai-codex-auth",
        "opencode-kimi-subscription",
        "@magpie-community/opencode-copilot-auth",
        "@magpie-community/opencode-gemini-web-auth",
        "@magpie-community/opencode-kimi-work-auth",
        "@magpie-community/opencode-grok-auth",
        "@magpie-community/opencode-cursor-auth",
        "@magpie-community/opencode-minimax-auth",
        "@magpie-community/opencode-qoder-auth",
        "@magpie-community/opencode-mimo-auth",
        "@magpie-community/opencode-doubao-work-auth",
        "magpie-aws-bedrock-sso",
    ]
    .iter()
    .position(|candidate| *candidate == package)
    .unwrap_or(usize::MAX)
}

pub(super) struct PluginMethod<'a> {
    pub provider: &'a str,
    pub name: &'a str,
    pub label: &'a str,
    pub method: usize,
    pub spec: &'a str,
    pub endpoint: &'a str,
    pub protocol: usize,
}

pub(super) fn plugin_methods<'a>(catalog: &'a Json, query: &str) -> Vec<PluginMethod<'a>> {
    let mut methods = Vec::new();
    for row in catalog.get("providers").array() {
        let provider = row.get("id").text();
        if provider.is_empty() {
            continue;
        }
        let mut name = crate::value(row, "name", provider);
        let mut endpoint = row.get("api").text();
        if !endpoint.starts_with("https://") {
            endpoint = "";
        }
        let mut protocol = match row.get("npm").text() {
            "@ai-sdk/anthropic" => 2,
            "@ai-sdk/google" | "@ai-sdk/google-vertex" => 3,
            _ => 0,
        };
        for (label, id, package, version, api, wire) in SUBSCRIPTION_PRESETS {
            if provider == id && row.get("spec").text() == format!("npm:{package}@{version}") {
                name = label;
                endpoint = api;
                protocol = wire;
                break;
            }
        }
        for (method, option) in row.get("methods").array().iter().enumerate() {
            let label = option.get("label").text();
            let description = format!("{name} {provider} {label}");
            if option.get("type").text() == "oauth" && endpoint_matches(query, &description) {
                methods.push(PluginMethod {
                    provider,
                    name,
                    label,
                    method,
                    spec: row.get("spec").text(),
                    endpoint,
                    protocol,
                });
            }
        }
    }
    methods
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

pub(super) fn open_browser(url: &str) -> Result<(), String> {
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
    base: String,
    plugin: String,
    method: Option<usize>,
    spec: String,
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

fn prompt_inputs(
    session: &mut Session,
    provider: &str,
    method: usize,
    sender: &mpsc::Sender<Reply>,
    answers: &mpsc::Receiver<String>,
) -> Result<Json, String> {
    let mut inputs = BTreeMap::new();
    let mut rejected = false;
    for _ in 0..32 {
        let params = Json::object([
            ("provider".into(), Json::String(provider.into())),
            ("method".into(), Json::Number(method.to_string())),
            ("inputs".into(), Json::Object(inputs.clone())),
        ]);
        let response = session.call("prompt", params)?;
        let prompt = response.get("prompt");
        if prompt == &Json::Null {
            return Ok(Json::Object(inputs));
        }
        let key = prompt.get("field").text();
        if key.is_empty() {
            return Err(tr("providers.oauth_prompt_unsupported").into());
        }
        let mut ui_prompt = prompt.clone();
        if let Json::Object(properties) = &mut ui_prompt {
            properties.insert("rejected".into(), Json::Bool(rejected));
        }
        sender
            .send(Reply {
                label: "OAuth prompt".into(),
                result: Ok(ui_prompt),
                refresh: false,
                starts_gateway: false,
            })
            .map_err(|_| tr("providers.oauth_cancelled"))?;
        let answer = answers
            .recv_timeout(Duration::from_secs(180))
            .map_err(|_| tr("providers.oauth_cancelled"))?;
        let validation = session.call(
            "validate",
            Json::object([
                ("provider".into(), Json::String(provider.into())),
                ("method".into(), Json::Number(method.to_string())),
                ("key".into(), Json::String(key.into())),
                ("value".into(), Json::String(answer.clone())),
            ]),
        )?;
        rejected = !validation.get("error").text().is_empty();
        if !rejected {
            inputs.insert(key.into(), Json::String(answer));
        }
    }
    Err(tr("providers.oauth_unavailable").into())
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
    if login.plugin == "lmm" {
        let domain = login
            .base
            .strip_suffix("/v1")
            .ok_or_else(|| tr("providers.oauth_invalid_domain").to_owned())?;
        configure_lmm(backend, domain, providers)?;
    }
    let mut session = Session::start(backend)?;
    let options = session.call("providers", Json::object([]))?;
    let provider = options
        .array()
        .iter()
        .find(|row| {
            row.get("id").text() == login.plugin
                && (login.spec.is_empty() || row.get("spec").text() == login.spec)
                && (login.plugin != "lmm" || row.get("spec").text() == LMM_REPO)
        })
        .ok_or_else(|| tr("providers.oauth_missing_plugin").to_owned())?;
    let methods = provider.get("methods").array();
    let method = login
        .method
        .or_else(|| {
            methods
                .iter()
                .position(|method| method.get("type").text() == "oauth")
        })
        .filter(|index| {
            methods
                .get(*index)
                .is_some_and(|method| method.get("type").text() == "oauth")
        })
        .ok_or_else(|| tr("providers.oauth_missing_plugin").to_owned())?;
    let inputs = prompt_inputs(&mut session, &login.plugin, method, sender, &code)?;
    let started = session.call(
        "authorize",
        Json::object([
            ("provider".into(), Json::String(login.plugin.clone())),
            ("method".into(), Json::Number(method.to_string())),
            ("inputs".into(), inputs),
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
    let paste_code = started.get("method").text() == "code";
    let _ = sender.send(Reply {
        label: if paste_code {
            "OAuth code"
        } else {
            "OAuth browser"
        }
        .into(),
        result: Ok(Json::String(started.get("instructions").text().to_owned())),
        refresh: false,
        starts_gateway: false,
    });
    if paste_code {
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
    let binding = Json::object([
        ("needs_endpoint".into(), Json::Bool(true)),
        ("name".into(), Json::String(name)),
        ("protocol".into(), Json::String(login.protocol.into())),
        ("plugin".into(), Json::String(provider.into())),
        ("account".into(), Json::String(account.into())),
    ]);
    let endpoint;
    let base = if login.base.is_empty() {
        endpoint = session
            .call(
                "providerEndpoint",
                Json::object([
                    ("provider".into(), Json::String(provider.into())),
                    ("account".into(), Json::String(account.into())),
                ]),
            )
            .unwrap_or_default();
        let url = endpoint.get("baseURL").text();
        if url.is_empty() {
            return Ok(binding);
        }
        url
    } else {
        &login.base
    };
    backend.run(&binding_args(&binding, base), None)
}

fn binding_args(binding: &Json, endpoint: &str) -> Vec<String> {
    words(&[
        "providers",
        "add",
        binding.get("name").text(),
        "--base-url",
        endpoint,
        "--protocol",
        binding.get("protocol").text(),
        "--plugin",
        binding.get("plugin").text(),
        "--plugin-account",
        binding.get("account").text(),
    ])
}

impl App {
    pub(super) fn start_oauth(&mut self) {
        let endpoint = if self.provider.url.trim().is_empty() && self.provider.plugin_id != "lmm" {
            Ok(self.provider.plugin_endpoint.clone())
        } else {
            issuer(&self.provider.url).map(|domain| format!("{domain}/v1"))
        };
        let base = match endpoint {
            Ok(base) => base,
            Err(error) => {
                self.notice = error;
                self.failed = true;
                return;
            }
        };
        if self.provider.pending_oauth_binding != Json::Null {
            let args = binding_args(&self.provider.pending_oauth_binding, &base);
            self.action("Save provider", args, None);
            self.repaint = true;
            return;
        }
        let plugin = self.provider.plugin_id.trim().to_owned();
        let name = if self.provider.name.trim().is_empty() {
            plugin.clone()
        } else {
            self.provider.name.trim().to_owned()
        };
        let login_options = Login {
            name,
            base,
            plugin,
            method: self.provider.oauth_method,
            spec: self.provider.oauth_spec.clone(),
            protocol: crate::protocols()[self.provider.protocol],
        };
        let (input, code) = mpsc::channel();
        self.oauth_input = Some(input);
        self.oauth_code.clear();
        self.oauth_instructions.clear();
        self.oauth_expects_code = false;
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
    fn picker_preserves_original_method_indexes_and_exact_endpoint() {
        let catalog = json::parse(r#"{"providers":[{"id":"fixture","name":"Fixture Provider","api":"https://api.fixture.test/special/v2","methods":[{"type":"api","label":"Key"},{"type":"oauth","label":"Browser"},{"type":"oauth","label":"Device"}]}]}"#).unwrap();
        let methods = plugin_methods(&catalog, "fixture");
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0].method, 1);
        assert_eq!(methods[1].method, 2);
        assert_eq!(methods[1].endpoint, "https://api.fixture.test/special/v2");
        assert!(plugin_methods(&catalog, "key").is_empty());
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
