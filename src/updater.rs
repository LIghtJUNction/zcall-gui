use crate::{
    VERSION,
    json::{self, Json},
};
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
fn sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = bytes.to_vec();
    let bits = (bytes.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bits.to_be_bytes());
    for block in data.as_chunks::<64>().0 {
        let mut words = [0u32; 64];
        for (i, c) in block.as_chunks::<4>().0.iter().enumerate() {
            words[i] = u32::from_be_bytes(*c);
        }
        for i in 16..64 {
            let a = words[i - 15];
            let b = words[i - 2];
            words[i] = words[i - 16]
                .wrapping_add(a.rotate_right(7) ^ a.rotate_right(18) ^ (a >> 3))
                .wrapping_add(words[i - 7])
                .wrapping_add(b.rotate_right(17) ^ b.rotate_right(19) ^ (b >> 10));
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let t1 = h
                .wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25))
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(words[i]);
            let t2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22))
                .wrapping_add((a & b) ^ (a & c) ^ (b & c));
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *s = s.wrapping_add(v);
        }
    }
    state.iter().map(|v| format!("{v:08x}")).collect()
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
    fn stable_versions_only() {
        assert!(version("v1.2.3").unwrap() > version("1.2.2").unwrap());
        for invalid in ["1.2", "1.2.3-rc", "../tag", "01.2.3", "1.2.3.4"] {
            assert!(version(invalid).is_err());
        }
    }
}
