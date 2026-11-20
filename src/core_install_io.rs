//! Bounded bootstrap I/O. Archive extraction belongs to zcalld-installer.
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

pub fn unique_name(prefix: &str) -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{prefix}-{}-{time:x}-{:x}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn canonical_future(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute()
        || path.to_str().is_none()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err("The installation location must be an absolute local path.".into());
    }
    if let Ok(metadata) = fs::symlink_metadata(path)
        && metadata.file_type().is_symlink()
    {
        return Err("The installer refuses a linked installation or cache location.".into());
    }
    let mut existing = path;
    let mut missing = Vec::new();
    while !existing.exists() {
        missing.push(
            existing
                .file_name()
                .ok_or("Invalid installation directory.")?
                .to_owned(),
        );
        existing = existing.parent().ok_or("Invalid installation directory.")?;
    }
    let mut result = existing
        .canonicalize()
        .map_err(|_| "Cannot resolve the installation directory.")?;
    for item in missing.into_iter().rev() {
        result.push(item);
    }
    Ok(result)
}

fn regular(path: &Path, directory: bool) -> Result<fs::Metadata, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "The private installer file is unavailable.")?;
    if metadata.file_type().is_symlink()
        || metadata.is_dir() != directory
        || (!directory && !metadata.is_file())
    {
        return Err("The installer refuses links or unexpected file types.".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("The installer refuses Windows reparse points.".into());
        }
    }
    Ok(metadata)
}

fn check_private(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = regular(path, directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        unsafe extern "C" {
            fn geteuid() -> u32;
        }
        // SAFETY: geteuid does not access caller memory.
        if metadata.uid() != unsafe { geteuid() }
            || metadata.mode() & 0o077 != 0
            || (!directory && metadata.nlink() != 1)
        {
            return Err("Installer files must be private to the current user.".into());
        }
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        windows_permissions(path, false)?;
    }
    Ok(())
}

pub fn directory(path: &Path) -> Result<(), String> {
    if path.exists() {
        return check_private(path, true);
    }
    let parent = path.parent().ok_or("Installer directory has no parent.")?;
    fs::create_dir_all(parent).map_err(|_| "Cannot create the installer cache parent.")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|_| "Cannot exclusively create the private installer directory.")?;
    }
    #[cfg(windows)]
    {
        fs::create_dir(path)
            .map_err(|_| "Cannot exclusively create the private installer directory.")?;
        windows_permissions(path, true)?;
    }
    check_private(path, true)
}

pub fn create(path: &Path) -> Result<File, String> {
    check_private(path.parent().ok_or("Installer file has no parent.")?, true)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|_| "Cannot exclusively create the installer file.")?;
    #[cfg(windows)]
    windows_permissions(path, true)?;
    check_private(path, false)?;
    Ok(file)
}

pub fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    check_private(path, false)?;
    let file = File::open(path).map_err(|_| "Cannot read the private installer file.")?;
    if file
        .metadata()
        .map_err(|_| "Cannot inspect the installer file.")?
        .len()
        > limit as u64
    {
        return Err("Installer data exceeds its size limit.".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read installer data.")?;
    if bytes.len() > limit {
        return Err("Installer data exceeds its size limit.".into());
    }
    Ok(bytes)
}

pub fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        check_private(path, false)?;
    }
    let parent = path.parent().ok_or("Installer state has no parent.")?;
    let temporary = parent.join(unique_name(".state"));
    let result = (|| {
        let mut file = create(&temporary)?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Cannot save installer state.")?;
        drop(file);
        fs::rename(&temporary, path).map_err(|_| "Cannot commit installer state.")?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| "Cannot sync installer state.")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

/// Serializes GUI state, including a helper operation that can outlive its window.
pub fn lock(directory: &Path) -> Result<File, String> {
    check_private(directory, true)?;
    let path = directory.join("gui.lock");
    let file = if path.exists() {
        check_private(&path, false)?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|_| "Cannot open the GUI installer lock.")?
    } else {
        create(&path)?
    };
    file.try_lock().map_err(
        |_| "Another installation operation is still running. Check its status when it finishes.",
    )?;
    check_private(&path, false)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let held = file
            .metadata()
            .map_err(|_| "Cannot inspect the installer lock.")?;
        let named = fs::symlink_metadata(&path).map_err(|_| "Installer lock disappeared.")?;
        if held.dev() != named.dev() || held.ino() != named.ino() {
            return Err("Installer lock changed while it was acquired.".into());
        }
    }
    Ok(file)
}

pub fn program(name: &str) -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").ok_or("The system search path is unavailable.")?;
    for directory in std::env::split_paths(&path).filter(|p| p.is_absolute()) {
        let path = directory.join(format!("{name}{}", if cfg!(windows) { ".exe" } else { "" }));
        if executable(&path) {
            return path
                .canonicalize()
                .map_err(|_| "Cannot resolve a required system program.".into());
        }
    }
    Err(format!("Required system program is unavailable: {name}"))
}

pub fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    let regular = metadata.is_file();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        regular && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(windows)]
    {
        regular
    }
}

pub fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for name in [
        "PATH",
        "HOME",
        "USERPROFILE",
        "XDG_DATA_HOME",
        "LOCALAPPDATA",
        "APPDATA",
        "SystemRoot",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "TMP",
        "TEMP",
        "TMPDIR",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "CURL_CA_BUNDLE",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
        "no_proxy",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .env("LC_ALL", "C");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00000200);
    }
    command
}

fn wait(
    command: &mut Command,
    output: Option<(&Path, u64)>,
    timeout: Duration,
) -> Result<bool, String> {
    let mut child = command
        .spawn()
        .map_err(|_| "Cannot start the required installer program.")?;
    let deadline = Instant::now() + timeout;
    loop {
        let too_large =
            output.is_some_and(|(path, limit)| fs::metadata(path).is_ok_and(|m| m.len() > limit));
        if too_large || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if too_large { "Installer output exceeds its size limit." } else {
                "The installer operation exceeded its deadline. Review its saved transaction before trying again."
            }.into());
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Cannot monitor the installer operation.".into());
            }
        }
    }
}

pub fn capture(
    program: &Path,
    args: &[String],
    output: &Path,
    limit: usize,
    timeout: Duration,
) -> Result<(bool, Vec<u8>), String> {
    let file = create(output)?;
    let mut cmd = command(program);
    cmd.args(args).stdout(Stdio::from(file));
    let success = wait(&mut cmd, Some((output, limit as u64)), timeout)?;
    drop(cmd);
    Ok((success, read(output, limit)?))
}

pub fn download(
    url: &str,
    accept: &str,
    destination: &Path,
    limit: usize,
) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://api.github.com/repos/LIghtJUNction/zcalld/") {
        return Err("Unexpected installer download origin.".into());
    }
    drop(create(destination)?);
    let mut cmd = command(&program("curl")?);
    cmd.args([
        "-q",
        "--fail",
        "--silent",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--max-redirs",
        "5",
        "--connect-timeout",
        "15",
        "--max-time",
        "120",
        "--max-filesize",
    ])
    .arg(limit.to_string())
    .args([
        "--header",
        &format!("Accept: {accept}"),
        "--user-agent",
        "zcall-gui-installer",
        "--output",
    ])
    .arg(destination)
    .args(["--url", url])
    .stdout(Stdio::null());
    if !wait(
        &mut cmd,
        Some((destination, limit as u64)),
        Duration::from_secs(125),
    )? {
        return Err("Could not download the official installer release.".into());
    }
    drop(cmd);
    read(destination, limit)
}

#[cfg(windows)]
fn windows_permissions(path: &Path, protect: bool) -> Result<(), String> {
    // Use the installed OS security APIs through PowerShell for this small
    // bootstrap. The native installer owns all archive/transaction ACL logic.
    const SCRIPT: &str = r#"$ErrorActionPreference='Stop'; $p=$env:ZCALLD_BOOTSTRAP_PATH; $item=Get-Item -LiteralPath $p -Force; if(($item.Attributes -band [IO.FileAttributes]::ReparsePoint)-ne 0){throw 'reparse'}; $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User; $system=[Security.Principal.SecurityIdentifier]::new('S-1-5-18'); if($env:ZCALLD_BOOTSTRAP_PROTECT -eq '1'){if($item.PSIsContainer){$acl=[Security.AccessControl.DirectorySecurity]::new()}else{$acl=[Security.AccessControl.FileSecurity]::new()}; $acl.SetOwner($sid); $acl.SetAccessRuleProtection($true,$false); foreach($who in @($sid,$system)){$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($who,[Security.AccessControl.FileSystemRights]::FullControl,[Security.AccessControl.AccessControlType]::Allow))}; Set-Acl -LiteralPath $p -AclObject $acl}; $acl=Get-Acl -LiteralPath $p; if(-not $acl.AreAccessRulesProtected -or $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $sid.Value){throw 'owner'}; $rules=@($acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])); if($rules.Count -ne 2){throw 'rules'}; $seen=@{}; foreach($rule in $rules){if($rule.IsInherited -or $rule.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow -or $rule.InheritanceFlags -ne [Security.AccessControl.InheritanceFlags]::None -or $rule.PropagationFlags -ne [Security.AccessControl.PropagationFlags]::None -or $rule.IdentityReference.Value -notin @($sid.Value,$system.Value)){throw 'access'}; $seen[$rule.IdentityReference.Value]=$true}; if(-not $seen.ContainsKey($sid.Value) -or -not $seen.ContainsKey($system.Value)){throw 'missing'}"#;
    let root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("SYSTEMROOT"))
        .ok_or("Windows system directory is unavailable.")?;
    let program = PathBuf::from(root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut cmd = command(&program);
    cmd.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        SCRIPT,
    ])
    .env("ZCALLD_BOOTSTRAP_PATH", path)
    .env("ZCALLD_BOOTSTRAP_PROTECT", if protect { "1" } else { "0" })
    .stdout(Stdio::null());
    if !wait(&mut cmd, None, Duration::from_secs(20))? {
        return Err(
            "Windows private installer permissions could not be established or verified.".into(),
        );
    }
    Ok(())
}
