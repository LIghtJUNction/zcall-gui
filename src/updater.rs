use crate::{
    VERSION,
    json::{self, Json},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
const REPOSITORY: &str = "https://github.com/LIghtJUNction/zcall-gui";
fn version(s: &str) -> Result<(u64, u64, u64), String> {
    let values = s
        .strip_prefix('v')
        .unwrap_or(s)
        .split('.')
        .collect::<Vec<_>>();
    if values.len() != 3
        || values.iter().any(|p| {
            p.is_empty()
                || (p.len() > 1 && p.starts_with('0'))
                || !p.bytes().all(|c| c.is_ascii_digit())
        })
    {
        return Err("Release version must be a stable major.minor.patch".into());
    }
    Ok((
        values[0].parse().map_err(|_| "Version overflow")?,
        values[1].parse().map_err(|_| "Version overflow")?,
        values[2].parse().map_err(|_| "Version overflow")?,
    ))
}
fn target() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        _ => Err("No published GUI binary for this platform".into()),
    }
}
fn curl(url: &str, destination: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut command = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" });
    command.args([
        "--disable",
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "15",
        "--max-time",
        "120",
        "--max-filesize",
        "67108864",
        "--user-agent",
        &format!("zcall-gui/{VERSION}"),
    ]);
    if let Some(path) = destination {
        command.arg("--output").arg(path);
    }
    command.arg(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .map_err(|e| format!("Cannot run system curl: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Release download failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(300)
                .collect::<String>()
        ));
    }
    if output.stdout.len() > 2 * 1024 * 1024 {
        return Err("Release metadata exceeds 2 MiB".into());
    }
    Ok(output.stdout)
}
fn release() -> Result<(String, String), String> {
    let bytes = curl(
        "https://api.github.com/repos/LIghtJUNction/zcall-gui/releases/latest",
        None,
    )?;
    let data =
        json::parse(std::str::from_utf8(&bytes).map_err(|_| "Release metadata is not UTF-8")?)?;
    let tag = data.get("tag_name").text();
    version(tag)?;
    if data.get("draft").boolean() || data.get("prerelease").boolean() {
        return Err("Only stable public releases can be installed".into());
    }
    let asset = format!(
        "zcall-gui-{tag}-{}{}",
        target()?,
        if cfg!(windows) { ".exe" } else { "" }
    );
    if !data
        .get("assets")
        .array()
        .iter()
        .any(|a| a.get("name").text() == asset)
    {
        return Err(format!("Release {tag} does not contain {asset}"));
    }
    Ok((tag.to_owned(), asset))
}
fn managed(path: &Path) -> bool {
    #[cfg(unix)]
    {
        path.starts_with("/usr") || path.starts_with("/opt")
    }
    #[cfg(windows)]
    {
        std::env::var_os("ProgramFiles").is_some_and(|p| path.starts_with(p))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        false
    }
}
pub fn check() -> Result<Json, String> {
    let (tag, _) = release()?;
    let available = version(&tag)? > version(VERSION)?;
    let packaged = managed(&std::env::current_exe().map_err(|e| e.to_string())?);
    Ok(Json::object([
        (
            "latest".into(),
            Json::String(tag.trim_start_matches('v').into()),
        ),
        ("available".into(), Json::Bool(available)),
        ("managed".into(), Json::Bool(packaged)),
        (
            "message".into(),
            Json::String(if packaged {
                "This installation is managed by the system. Update with your package manager."
                    .into()
            } else if available {
                format!("zcall-gui {tag} is available.")
            } else {
                "zcall-gui is current.".into()
            }),
        ),
    ]))
}
pub fn apply() -> Result<Json, String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    if managed(&current) {
        return Err(
            "System-managed installation: use your package manager (Arch: yay -Syu zcall-gui-bin)."
                .into(),
        );
    }
    let (tag, asset) = release()?;
    if version(&tag)? <= version(VERSION)? {
        return check();
    }
    let parent = current
        .parent()
        .ok_or("Executable has no parent directory")?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let stage = parent.join(format!(".zcall-gui-update-{}-{stamp}", std::process::id()));
    fs::create_dir(&stage)
        .map_err(|e| format!("Cannot stage an update beside the installed executable: {e}"))?;
    let result = (|| {
        let url = format!("{REPOSITORY}/releases/download/{tag}/{asset}");
        let checksum = curl(&(url.clone() + ".sha256"), None)?;
        let checksum = String::from_utf8(checksum).map_err(|_| "Invalid checksum response")?;
        let expected = checksum
            .split_whitespace()
            .next()
            .ok_or("Empty SHA-256 checksum")?;
        if expected.len() != 64 || !expected.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("Malformed SHA-256 checksum".into());
        }
        let file = stage.join(if cfg!(windows) { "new.exe" } else { "new" });
        curl(&url, Some(&file))?;
        if fs::metadata(&file).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
            return Err("Release binary exceeds 64 MiB".into());
        }
        let bytes = fs::read(&file).map_err(|e| e.to_string())?;
        if sha256(&bytes) != expected.to_ascii_lowercase() {
            return Err("SHA-256 mismatch; installed binary was not changed".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        let output = Command::new(&file)
            .arg("--version")
            .output()
            .map_err(|e| format!("Downloaded executable did not start: {e}"))?;
        let expected_version = format!("zcall-gui {}", tag.trim_start_matches('v'));
        if !output.status.success()
            || String::from_utf8_lossy(&output.stdout).trim() != expected_version
        {
            return Err("Downloaded executable has an unexpected version".into());
        }
        #[cfg(windows)]
        {
            let helper = stage.join("helper.exe");
            fs::copy(&file, &helper).map_err(|e| e.to_string())?;
            use std::os::windows::process::CommandExt;
            Command::new(&helper)
                .arg("--complete-update")
                .arg(&current)
                .arg("--from")
                .arg(&file)
                .arg("--wait-pid")
                .arg(std::process::id().to_string())
                .arg("--sha256")
                .arg(expected)
                .creation_flags(0x08000000)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(windows))]
        {
            replace(&file, &current)?;
        }
        Ok(Json::object([
            (
                "latest".into(),
                Json::String(tag.trim_start_matches('v').into()),
            ),
            ("exit_required".into(), Json::Bool(cfg!(windows))),
            (
                "message".into(),
                Json::String(
                    if cfg!(windows) {
                        "Update verified. Closing and reopening the GUI to finish."
                    } else {
                        "Update installed. Close and reopen the GUI to use the new version."
                    }
                    .into(),
                ),
            ),
        ]))
    })();
    if !cfg!(windows) || result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}
fn replace(new: &Path, current: &Path) -> Result<(), String> {
    let backup = PathBuf::from(format!("{}.previous", current.display()));
    fs::copy(current, &backup).map_err(|e| format!("Cannot retain previous executable: {e}"))?;
    #[cfg(windows)]
    {
        fs::remove_file(current).map_err(|e| format!("Cannot replace running GUI: {e}"))?;
    }
    if let Err(error) = fs::rename(new, current) {
        #[cfg(windows)]
        {
            let _ = fs::copy(&backup, current);
        }
        return Err(format!("Cannot activate verified update: {error}"));
    }
    Ok(())
}
pub fn complete_if_requested() -> Result<bool, String> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().is_none_or(|a| a != "--complete-update") {
        return Ok(false);
    }
    #[cfg(not(windows))]
    {
        Err("The deferred update helper is Windows-only".into())
    }
    #[cfg(windows)]
    {
        if args.len() != 8
            || args[2] != "--from"
            || args[4] != "--wait-pid"
            || args[6] != "--sha256"
        {
            return Err("Invalid deferred-update arguments".into());
        }
        let current = PathBuf::from(&args[1]);
        let new = PathBuf::from(&args[3]);
        let pid = args[5]
            .to_str()
            .ok_or("Invalid parent PID")?
            .parse::<u32>()
            .map_err(|_| "Invalid parent PID")?;
        let parent = current.parent().ok_or("No update target directory")?;
        let stage = new.parent().ok_or("No update staging directory")?;
        if stage.parent() != Some(parent)
            || !stage
                .file_name()
                .is_some_and(|s| s.to_string_lossy().starts_with(".zcall-gui-update-"))
        {
            return Err("Update staging directory does not match target".into());
        }
        let bytes = fs::read(&new).map_err(|e| e.to_string())?;
        if sha256(&bytes) != args[7].to_string_lossy() {
            return Err("Deferred-update checksum mismatch".into());
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
            fn WaitForSingleObject(handle: *mut std::ffi::c_void, timeout: u32) -> u32;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }
        unsafe {
            let handle = OpenProcess(0x00100000, 0, pid);
            if !handle.is_null() {
                let result = WaitForSingleObject(handle, 60000);
                CloseHandle(handle);
                if result != 0 {
                    return Err("Previous GUI did not exit within 60 seconds".into());
                }
            }
        }
        replace(&new, &current)?;
        Command::new(&current).spawn().map_err(|e| e.to_string())?;
        Ok(true)
    }
}
pub(crate) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = [0; 64];
    zhex::encode_lower(&digest, &mut text).expect("SHA-256 hex output capacity");
    String::from_utf8(text.to_vec()).expect("zhex produces ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
    #[test]
    fn sha256_binary_input_and_block_boundaries() {
        for (length, expected) in [
            (
                1,
                "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d",
            ),
            (
                55,
                "463eb28e72f82e0a96c0a4cc53690c571281131f672aa229e0d45ae59b598b59",
            ),
            (
                56,
                "da2ae4d6b36748f2a318f23e7ab1dfdf45acdc9d049bd80e59de82a60895f562",
            ),
            (
                63,
                "29af2686fd53374a36b0846694cc342177e428d1647515f078784d69cdb9e488",
            ),
            (
                64,
                "fdeab9acf3710362bd2658cdc9a29e8f9c757fcf9811603a8c447cd1d9151108",
            ),
            (
                65,
                "4bfd2c8b6f1eec7a2afeb48b934ee4b2694182027e6d0fc075074f2fabb31781",
            ),
            (
                127,
                "92ca0fa6651ee2f97b884b7246a562fa71250fedefe5ebf270d31c546bfea976",
            ),
            (
                128,
                "471fb943aa23c511f6f72f8d1652d9c880cfa392ad80503120547703e56a2be5",
            ),
            (
                129,
                "5099c6a56203f9687f7d33f4bfdf576d31dc91f6b695ecea38b2770c87631135",
            ),
            (
                255,
                "3f8591112c6bbe5c963965954e293108b7208ed2af893e500d859368c654eabe",
            ),
            (
                256,
                "40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880",
            ),
            (
                257,
                "54acfbfedc4d8da40f76f275e1a98f10af8ef1fb9fb39e5a67a00aabcbe6597c",
            ),
        ] {
            let bytes: Vec<u8> = (0..length).map(|index| index as u8).collect();
            assert_eq!(sha256(&bytes), expected, "length {length}");
        }
    }
    #[test]
    fn stable_versions_only() {
        assert!(version("v1.2.3").unwrap() > version("1.2.2").unwrap());
        for invalid in ["1.2", "1.2.3-rc", "../tag", "01.2.3", "1.2.3.4"] {
            assert!(version(invalid).is_err());
        }
    }
}
