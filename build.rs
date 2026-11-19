use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=packaging/zcall-gui.ico");
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let icon = root
        .join("packaging/zcall-gui.ico")
        .to_string_lossy()
        .replace('\\', "/");
    let resource = out.join("zcall-gui.rc");
    fs::write(&resource, format!("1 ICON \"{icon}\"\n")).expect("write Windows icon resource");
    let output = out.join("zcall-gui.res");
    let status = Command::new("rc")
        .args(["/nologo", "/fo"])
        .arg(&output)
        .arg(&resource)
        .status()
        .expect("MSVC resource compiler is required");
    assert!(status.success(), "Windows icon resource compilation failed");
    println!("cargo:rustc-link-arg-bins={}", output.display());
}
