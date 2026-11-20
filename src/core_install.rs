//! The GUI verifies a small official helper; that helper owns all bundle changes.
use crate::{
    core_install_io as io,
    json::{self, Json},
    updater,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const API: &str = "https://api.github.com/repos/LIghtJUNction/zcalld/";
const METADATA_LIMIT: usize = 1024 * 1024;
const HELPER_LIMIT: usize = 32 * 1024 * 1024;

fn string(value: &Json, key: &str) -> Result<String, String> {
    match value.get(key) {
        Json::String(value) if !value.is_empty() => Ok(value.clone()),
        _ => Err(format!("Installer response is missing {key}.")),
    }
}
fn number(value: &Json, key: &str) -> Result<u64, String> {
    match value.get(key) {
        Json::Number(value) => value
            .parse()
            .map_err(|_| format!("Installer response has an invalid {key}.")),
        _ => Err(format!("Installer response is missing {key}.")),
    }
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn version(value: &str) -> Result<(), String> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || p.len() > 8
                || (p.len() > 1 && p.starts_with('0'))
                || !p.bytes().all(|c| c.is_ascii_digit())
        })
    {
        return Err("The official release must have a stable major.minor.patch version.".into());
    }
    Ok(())
}
fn parse(bytes: &[u8]) -> Result<Json, String> {
    json::parse(std::str::from_utf8(bytes).map_err(|_| "Installer data is not UTF-8.")?)
}
fn put(value: &mut Json, key: &str, entry: Json) {
    if let Json::Object(object) = value {
        object.insert(key.into(), entry);
    }
}
fn text(value: impl Into<String>) -> Json {
    Json::String(value.into())
}
fn path_text(value: &Path) -> Result<String, String> {
    value
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| "Installer paths must be UTF-8.".into())
}
fn helper_name() -> &'static str {
    if cfg!(windows) {
        "zcalld-installer.exe"
    } else {
        "zcalld-installer"
    }
}
fn binary_name() -> &'static str {
    if cfg!(windows) {
        "zcalld.exe"
    } else {
        "zcalld"
    }
}
fn target() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        _ => Err("No native gateway installer is published for this platform.".into()),
    }
}

fn cache_root() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")));
    io::canonical_future(
        &base
            .ok_or("Cannot find the private installation directory.")?
            .join("zcall-gui/core-installer"),
    )
}

fn default_prefix() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")));
    io::canonical_future(&base?.join("zcalld/runtime")).ok()
}

pub fn resolve_binary(binary: &str) -> Option<PathBuf> {
    let path = Path::new(binary);
    if path.components().count() > 1 {
        return io::executable(path)
            .then(|| path.canonicalize().ok())
            .flatten();
    }
    let search = std::env::var_os("PATH")?;
    std::env::split_paths(&search)
        .filter(|p| p.is_absolute())
        .find_map(|dir| {
            let path = dir.join(if cfg!(windows) && !binary.ends_with(".exe") {
                format!("{binary}.exe")
            } else {
                binary.into()
            });
            io::executable(&path)
                .then(|| path.canonicalize().ok())
                .flatten()
        })
}

pub fn discover_default() -> Option<String> {
    let prefix = default_prefix()?;
    let receipt =
        parse(&io::read(&prefix.join(".zcalld-install.json"), METADATA_LIMIT).ok()?).ok()?;
    if receipt.get("owner").text() != "zcalld-installer"
        || receipt.get("schema").text() != "1"
        || Path::new(receipt.get("plan").get("prefix").text()) != prefix
    {
        return None;
    }
    let binary = prefix.join("bin").join(binary_name());
    io::executable(&binary).then(|| binary.to_string_lossy().into_owned())
}

fn managed(binary: &Path) -> bool {
    #[cfg(unix)]
    {
        [
            "/usr",
            "/bin",
            "/sbin",
            "/lib",
            "/lib64",
            "/opt/homebrew",
            "/nix/store",
            "/Applications",
        ]
        .iter()
        .any(|base| binary.starts_with(base))
    }
    #[cfg(windows)]
    {
        let path = binary
            .to_string_lossy()
            .trim_start_matches("\\\\?\\")
            .to_ascii_lowercase();
        ["ProgramFiles", "ProgramFiles(x86)", "SystemRoot", "WINDIR"]
            .iter()
            .filter_map(std::env::var_os)
            .any(|base| {
                let base = base
                    .to_string_lossy()
                    .trim_start_matches("\\\\?\\")
                    .to_ascii_lowercase();
                path == base || path.starts_with(&(base + "\\"))
            })
    }
}
pub fn package_managed(binary: &str) -> bool {
    resolve_binary(binary).as_deref().is_some_and(managed)
}

fn checksum_list(bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    if bytes.len() > METADATA_LIMIT {
        return Err("Official checksums exceed their size limit.".into());
    }
    let mut values = BTreeMap::new();
    for line in std::str::from_utf8(bytes)
        .map_err(|_| "Official checksums are not UTF-8.")?
        .lines()
    {
        let (digest, name) = line
            .split_once("  ")
            .ok_or("Malformed official checksum list.")?;
        if !hex(digest, 64)
            || name.is_empty()
            || name.len() > 384
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
            || values.len() >= 128
            || values.insert(name.into(), digest.into()).is_some()
        {
            return Err("Official checksums contain invalid or duplicate entries.".into());
        }
    }
    if values.is_empty() {
        return Err("The official checksum list is empty.".into());
    }
    Ok(values)
}

fn asset<'a>(release: &'a Json, name: &str) -> Result<&'a Json, String> {
    let entries = release
        .get("assets")
        .array()
        .iter()
        .filter(|a| a.get("name").text() == name)
        .collect::<Vec<_>>();
    if entries.len() != 1 {
        return Err(format!(
            "The selected release does not provide one complete installer asset: {name}"
        ));
    }
    Ok(entries[0])
}
fn api(directory: &Path, suffix: &str, file: &str) -> Result<Json, String> {
    parse(&io::download(
        &format!("{API}{suffix}"),
        "application/vnd.github+json",
        &directory.join(file),
        METADATA_LIMIT,
    )?)
}

#[derive(Clone)]
struct Session {
    directory: PathBuf,
    version: String,
    target: String,
    source: String,
    helper_sha256: String,
    plan_sha256: String,
    prefix: PathBuf,
    transaction: Option<PathBuf>,
    pending_output: Option<String>,
    last: Json,
}
impl Session {
    fn helper(&self) -> PathBuf {
        self.directory.join(helper_name())
    }
    fn plan(&self) -> PathBuf {
        self.directory.join("plan.json")
    }
    fn value(&self) -> Result<Json, String> {
        Ok(Json::object([
            ("schema".into(), Json::Number("1".into())),
            ("directory".into(), text(path_text(&self.directory)?)),
            ("version".into(), text(self.version.clone())),
            ("target".into(), text(self.target.clone())),
            ("source".into(), text(self.source.clone())),
            ("helper_sha256".into(), text(self.helper_sha256.clone())),
            ("plan_sha256".into(), text(self.plan_sha256.clone())),
            ("prefix".into(), text(path_text(&self.prefix)?)),
            (
                "transaction".into(),
                self.transaction
                    .as_deref()
                    .map(path_text)
                    .transpose()?
                    .map(text)
                    .unwrap_or(Json::Null),
            ),
            (
                "pending_output".into(),
                self.pending_output.clone().map(text).unwrap_or(Json::Null),
            ),
            ("last".into(), self.last.clone()),
        ]))
    }
    fn save(&self) -> Result<(), String> {
        io::atomic(
            &cache_root()?.join("latest.json"),
            self.value()?.stringify().as_bytes(),
        )
    }
    fn verify(&self) -> Result<(), String> {
        version(&self.version)?;
        if self.target != target()?
            || !hex(&self.source, 40)
            || !hex(&self.helper_sha256, 64)
            || !hex(&self.plan_sha256, 64)
            || self.directory.parent() != Some(cache_root()?.as_path())
            || !self
                .directory
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| {
                    s.starts_with("prepare-")
                        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
        {
            return Err("Saved installer identity is invalid.".into());
        }
        io::directory(&self.directory)?;
        if updater::sha256(&io::read(&self.helper(), HELPER_LIMIT)?) != self.helper_sha256 {
            return Err(
                "The verified installation helper has changed. No installation was started.".into(),
            );
        }
        if updater::sha256(&io::read(&self.plan(), METADATA_LIMIT)?) != self.plan_sha256 {
            return Err(
                "The approved installation plan has changed. No new operation was started.".into(),
            );
        }
        if let Some(path) = &self.transaction {
            self.transaction_path(&path_text(path)?)?;
        }
        if let Some(name) = &self.pending_output
            && (!name.starts_with("operation-")
                || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
        {
            return Err("Saved installer output path is invalid.".into());
        }
        Ok(())
    }
    fn transaction_path(&self, value: &str) -> Result<PathBuf, String> {
        let path = PathBuf::from(value);
        let name = self
            .prefix
            .file_name()
            .and_then(|p| p.to_str())
            .ok_or("Invalid install prefix.")?;
        let parent = self
            .prefix
            .parent()
            .ok_or("Invalid install prefix.")?
            .join(format!(".{name}.zcalld-installer/transactions"));
        if path.parent() != Some(parent.as_path())
            || !path
                .file_name()
                .and_then(|p| p.to_str())
                .is_some_and(|p| hex(p, 32))
        {
            return Err("Installer transaction does not belong to the selected location.".into());
        }
        Ok(path)
    }
    fn find_transaction(&self) -> Result<Option<PathBuf>, String> {
        let name = self
            .prefix
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or("Invalid install prefix.")?;
        let parent = self
            .prefix
            .parent()
            .ok_or("Invalid install prefix.")?
            .join(format!(".{name}.zcalld-installer/transactions"));
        if !parent.exists() {
            return Ok(None);
        }
        io::directory(&parent)?;
        let mut found = None;
        let mut count = 0;
        for entry in
            fs::read_dir(&parent).map_err(|_| "Cannot read saved installation transactions.")?
        {
            let entry = entry.map_err(|_| "Cannot inspect a saved installation transaction.")?;
            count += 1;
            if count > 64 {
                return Err("Too many saved installation transactions to recover safely.".into());
            }
            let path = self.transaction_path(&path_text(&entry.path())?)?;
            io::directory(&path)?;
            let plan = io::read(&path.join("plan.json"), METADATA_LIMIT)?;
            if updater::sha256(&plan) == self.plan_sha256 {
                if found.is_some() {
                    return Err("More than one transaction matches this approved plan. Inspect its recovery records.".into());
                }
                found = Some(path);
            }
        }
        Ok(found)
    }
    fn ingest(&mut self, mut result: Json) -> Result<Json, String> {
        if !matches!(result.get("ok"), Json::Bool(_)) {
            return Err("The installer returned an invalid operation result.".into());
        }
        if let Json::String(value) = result.get("transaction") {
            self.transaction = Some(self.transaction_path(value)?);
        } else if let Some((_, value)) = result
            .get("error")
            .text()
            .rsplit_once("; installation journal: ")
        {
            // The CLI supplies this stable, bounded recovery location on errors.
            self.transaction = Some(self.transaction_path(value)?);
        }
        if result.get("committed").boolean()
            && (result.get("phase").text() != "committed"
                || result.get("version").text() != self.version
                || result.get("target").text() != self.target
                || result.get("source").text() != self.source
                || Path::new(result.get("prefix").text()) != self.prefix
                || Path::new(result.get("binary_path").text())
                    != self.prefix.join("bin").join(binary_name()))
        {
            return Err("The installer completion does not match the approved plan.".into());
        }
        if result.get("phase").text().is_empty() {
            put(&mut result, "phase", text("failed"));
        }
        put(&mut result, "latest", text(self.version.clone()));
        put(&mut result, "prefix", text(path_text(&self.prefix)?));
        put(&mut result, "administrator_required", Json::Bool(false));
        self.last = result.clone();
        self.pending_output = None;
        self.save()?;
        Ok(result)
    }
    fn call(&mut self, args: &[String], timeout: Duration) -> Result<Json, String> {
        self.verify()?;
        let output = io::unique_name("operation");
        self.pending_output = Some(output.clone());
        self.save()?;
        let file = self.directory.join(output);
        let response: Result<Json, String> = (|| {
            let (success, bytes) =
                io::capture(&self.helper(), args, &file, METADATA_LIMIT, timeout)?;
            let value = parse(&bytes)?;
            if success != value.get("ok").boolean() {
                return Err("Installer exit status does not match its result.".into());
            }
            let result = self.ingest(value)?;
            let _ = fs::remove_file(file);
            Ok(result)
        })();
        if let Err(error) = &response {
            // capture has reaped its owned child on every return. A failed spawn
            // or bounded-output failure must not masquerade as a running helper.
            if self.transaction.is_none() {
                self.transaction = self.find_transaction()?;
            }
            self.pending_output = None;
            self.last = Json::object([
                ("ok".into(), Json::Bool(false)),
                ("phase".into(), text("failed")),
                ("error".into(), text(error.clone())),
                ("version".into(), text(self.version.clone())),
                ("prefix".into(), text(path_text(&self.prefix)?)),
                (
                    "transaction".into(),
                    self.transaction
                        .as_deref()
                        .map(path_text)
                        .transpose()?
                        .map(text)
                        .unwrap_or(Json::Null),
                ),
            ]);
            self.save()?;
        }
        response
    }
}

fn load() -> Result<Option<Session>, String> {
    let root = cache_root()?;
    let path = root.join("latest.json");
    if !path.exists() {
        return Ok(None);
    }
    io::directory(&root)?;
    let value = parse(&io::read(&path, METADATA_LIMIT)?)?;
    if number(&value, "schema")? != 1 {
        return Err("Unsupported GUI installer state.".into());
    }
    let optional = |name| match value.get(name) {
        Json::Null => Ok(None),
        Json::String(v) => Ok(Some(v.clone())),
        _ => Err("Invalid GUI installer state.".to_owned()),
    };
    let mut session = Session {
        directory: string(&value, "directory")?.into(),
        version: string(&value, "version")?,
        target: string(&value, "target")?,
        source: string(&value, "source")?,
        helper_sha256: string(&value, "helper_sha256")?,
        plan_sha256: string(&value, "plan_sha256")?,
        prefix: string(&value, "prefix")?.into(),
        transaction: optional("transaction")?.map(PathBuf::from),
        pending_output: optional("pending_output")?,
        last: value.get("last").clone(),
    };
    session.verify()?;
    if let Some(output) = session.pending_output.clone() {
        let path = session.directory.join(output);
        if path.exists()
            && let Ok(bytes) = io::read(&path, METADATA_LIMIT)
            && let Ok(value) = parse(&bytes)
            && matches!(value.get("ok"), Json::Bool(_))
        {
            session.ingest(value)?;
            let _ = fs::remove_file(path);
        } else if let Some(transaction) = session.find_transaction()? {
            // The old window may have exited before reading the helper result.
            // A byte-identical approved plan binds recovery; the core transaction
            // lock still rejects changes while a surviving helper is busy.
            session.transaction = Some(transaction);
            session.pending_output = None;
            session.save()?;
        }
    }
    Ok(Some(session))
}

pub fn prepare(current: &str) -> Result<Json, String> {
    let current = resolve_binary(current);
    if current.as_deref().is_some_and(managed) {
        return Err("This gateway belongs to the operating-system package manager. Use its package update action.".into());
    }
    let root = cache_root()?;
    io::directory(&root)?;
    let _lock = io::lock(&root)?;
    if let Some(session) = load()? {
        let phase = session.last.get("phase").text();
        if session.pending_output.is_some()
            || !matches!(phase, "planned" | "committed" | "rolled_back" | "failed")
        {
            return Err(
                "Finish or roll back the previous installation before preparing another update."
                    .into(),
            );
        }
    }
    if fs::read_dir(&root)
        .map_err(|_| "Cannot inspect the private installer cache.")?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("prepare-"))
        .take(17)
        .count()
        >= 16
    {
        return Err("The installer cache contains retained recovery records. Inspect them before preparing another update.".into());
    }
    let directory = root.join(io::unique_name("prepare"));
    io::directory(&directory)?;
    let release = api(&directory, "releases/latest", "release.json")?;
    if release.get("draft") != &Json::Bool(false) || release.get("prerelease") != &Json::Bool(false)
    {
        return Err("Only a published stable gateway release can be installed.".into());
    }
    let tag = string(&release, "tag_name")?;
    let version_text = tag
        .strip_prefix('v')
        .ok_or("Malformed official release tag.")?
        .to_owned();
    version(&version_text)?;
    let platform = target()?;
    let helper_asset = format!(
        "zcalld-installer-{version_text}-{platform}{}",
        if cfg!(windows) { ".exe" } else { "" }
    );
    let archive_asset = format!(
        "zcalld-{version_text}-{platform}{}",
        if cfg!(windows) { ".zip" } else { ".tar.gz" }
    );
    let helper = asset(&release, &helper_asset)?;
    let archive = asset(&release, &archive_asset)?;
    if number(&release, "id")? == 0 || number(helper, "id")? == 0 || number(archive, "id")? == 0 {
        return Err("The official release has an invalid asset identity.".into());
    }
    let helper_size = number(helper, "size")?;
    if helper_size == 0 || helper_size > HELPER_LIMIT as u64 {
        return Err("Official installer size is invalid.".into());
    }
    let checks = asset(&release, "SHA256SUMS")?;
    if number(checks, "id")? == 0 {
        return Err("The official checksum asset identity is invalid.".into());
    }
    let checks = checksum_list(&io::download(
        &format!("{API}releases/assets/{}", number(checks, "id")?),
        "application/octet-stream",
        &directory.join("SHA256SUMS"),
        METADATA_LIMIT,
    )?)?;
    let expected = checks
        .get(&helper_asset)
        .ok_or("The official helper checksum is missing.")?
        .clone();
    let archive_sha = checks
        .get(&archive_asset)
        .ok_or("The official archive checksum is missing.")?;
    let reference = api(&directory, &format!("git/ref/tags/{tag}"), "reference.json")?;
    if reference.get("ref").text() != format!("refs/tags/{tag}") {
        return Err("The official tag does not match the selected release.".into());
    }
    let mut object = reference.get("object").clone();
    if object.get("type").text() == "tag" {
        let sha = string(&object, "sha")?;
        if !hex(&sha, 40) {
            return Err("Malformed official tag identity.".into());
        }
        object = api(&directory, &format!("git/tags/{sha}"), "tag.json")?
            .get("object")
            .clone();
    }
    let source = string(&object, "sha")?;
    if object.get("type").text() != "commit" || !hex(&source, 40) {
        return Err("The official release is not bound to a source commit.".into());
    }
    let helper_path = directory.join(helper_name());
    let bytes = io::download(
        &format!("{API}releases/assets/{}", number(helper, "id")?),
        "application/octet-stream",
        &helper_path,
        HELPER_LIMIT,
    )?;
    if bytes.len() as u64 != helper_size || updater::sha256(&bytes) != expected {
        return Err(
            "Official helper SHA-256 or size does not match. Nothing was installed.".into(),
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&helper_path, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Cannot make the verified helper executable.")?;
    }
    let probe = directory.join("version.out");
    let (success, bytes) = io::capture(
        &helper_path,
        &["--version".into()],
        &probe,
        4096,
        Duration::from_secs(5),
    )?;
    if !success
        || std::str::from_utf8(&bytes).unwrap_or("").trim()
            != format!("zcalld-installer {version_text}")
    {
        return Err("The verified helper has an unexpected executable version.".into());
    }
    let (success, bytes) = io::capture(
        &helper_path,
        &["defaults".into()],
        &directory.join("defaults.out"),
        METADATA_LIMIT,
        Duration::from_secs(10),
    )?;
    let defaults = parse(&bytes)?;
    if !success
        || !defaults.get("ok").boolean()
        || defaults.get("version").text() != version_text
        || defaults.get("target").text() != platform
        || defaults.get("protocol").text() != "1"
        || !defaults.get("full_archive").boolean()
        || !defaults.get("runtime_lease").boolean()
    {
        return Err("This release does not support complete native bundle installation.".into());
    }
    let mut prefix = PathBuf::from(string(&defaults, "prefix")?);
    if let Some(binary) = current
        && let Some(bin) = binary.parent()
        && bin.file_name().and_then(|p| p.to_str()) == Some("bin")
        && let Some(parent) = bin.parent()
        && parent.join(".zcalld-install.json").is_file()
    {
        // The helper must validate ownership and every existing file before
        // changing this location. Legacy shared directories are never adopted.
        prefix = parent.to_path_buf();
    }
    let prefix = io::canonical_future(&prefix)?;
    let plan = directory.join("plan.json");
    let (success, bytes) = io::capture(
        &helper_path,
        &[
            "plan".into(),
            "--version".into(),
            version_text.clone(),
            "--prefix".into(),
            path_text(&prefix)?,
            "--out".into(),
            path_text(&plan)?,
        ],
        &directory.join("plan.out"),
        METADATA_LIMIT,
        Duration::from_secs(240),
    )?;
    let mut result = parse(&bytes)?;
    if !success || !result.get("ok").boolean() {
        return Err(result.get("error").text().to_owned());
    }
    if result.get("version").text() != version_text
        || result.get("target").text() != platform
        || result.get("source").text() != source
        || Path::new(result.get("prefix").text()) != prefix
        || result.get("archive_name").text() != archive_asset
        || result.get("archive_sha256").text() != archive_sha
        || number(&result, "archive_asset_id")? != number(archive, "id")?
        || number(&result, "release_id")? != number(&release, "id")?
    {
        return Err("The official release changed while the plan was prepared. No installation was started.".into());
    }
    put(&mut result, "latest", text(version_text.clone()));
    put(&mut result, "administrator_required", Json::Bool(false));
    put(&mut result, "full_archive", Json::Bool(true));
    let plan_sha256 = updater::sha256(&io::read(&plan, METADATA_LIMIT)?);
    let session = Session {
        directory,
        version: version_text,
        target: platform.into(),
        source,
        helper_sha256: expected,
        plan_sha256,
        prefix,
        transaction: None,
        pending_output: None,
        last: result.clone(),
    };
    session.save()?;
    Ok(result)
}

pub fn status() -> Result<Json, String> {
    let root = cache_root()?;
    if !root.exists() {
        return Ok(Json::Null);
    }
    io::directory(&root)?;
    let _lock = io::lock(&root)?;
    let Some(mut session) = load()? else {
        return Ok(Json::Null);
    };
    if session.pending_output.is_some() {
        return Ok(Json::object([
            ("ok".into(), Json::Bool(true)),
            ("phase".into(), text("pending_operation")),
            ("committed".into(), Json::Bool(false)),
            ("prefix".into(), text(path_text(&session.prefix)?)),
        ]));
    }
    if let Some(transaction) = &session.transaction {
        return session.call(
            &[
                "status".into(),
                "--transaction".into(),
                path_text(transaction)?,
            ],
            Duration::from_secs(15),
        );
    }
    Ok(session.last)
}

pub fn apply() -> Result<Json, String> {
    let root = cache_root()?;
    io::directory(&root)?;
    let _lock = io::lock(&root)?;
    let mut session = load()?.ok_or("Prepare an installation plan first.")?;
    if session.pending_output.is_some() || session.last.get("phase").text() != "planned" {
        return Err("This plan has already been used. Check its installation status.".into());
    }
    let plan = session.plan();
    session.call(
        &[
            "apply".into(),
            "--plan".into(),
            path_text(&plan)?,
            "--defer".into(),
        ],
        Duration::from_secs(420),
    )
}

pub fn recover(rollback: bool) -> Result<Json, String> {
    let root = cache_root()?;
    io::directory(&root)?;
    let _lock = io::lock(&root)?;
    let mut session = load()?.ok_or("There is no saved installation transaction.")?;
    if session.pending_output.is_some() {
        return Err(
            "The previous installer operation has not finished. Check its status first.".into(),
        );
    }
    let transaction = path_text(
        session
            .transaction
            .as_deref()
            .ok_or("The installation has no recovery transaction yet.")?,
    )?;
    let args = if rollback {
        vec!["rollback".into(), "--transaction".into(), transaction]
    } else {
        vec![
            "resume".into(),
            "--transaction".into(),
            transaction,
            "--wait-ms".into(),
            "120000".into(),
        ]
    };
    session.call(&args, Duration::from_secs(150))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_checksums_reject_ambiguity_and_paths() {
        let digest = "a".repeat(64);
        assert_eq!(
            checksum_list(format!("{digest}  helper\n").as_bytes()).unwrap()["helper"],
            digest
        );
        for bad in [
            format!("{digest}  helper\n{digest}  helper\n"),
            format!("{digest}  ../helper\n"),
            format!("{digest}  C:\\helper\n"),
        ] {
            assert!(checksum_list(bad.as_bytes()).is_err());
        }
    }
    #[test]
    fn fixed_release_rejects_nonstable_or_duplicate_assets() {
        for text in ["v1.2.3", "1.2", "1.2.3-rc", "01.2.3", "../latest"] {
            assert!(version(text).is_err());
        }
        let value = json::parse(r#"{"assets":[{"name":"helper"},{"name":"helper"}]}"#).unwrap();
        assert!(asset(&value, "helper").is_err());
    }
    #[test]
    fn saved_state_roundtrips_paths_and_control_characters() {
        let original = Json::object([
            ("path".into(), text("C:\\Users\\测试\\test\"name\n")),
            ("empty".into(), Json::Null),
            ("pending".into(), Json::Bool(true)),
        ]);
        assert_eq!(json::parse(&original.stringify()).unwrap(), original);
    }
}
