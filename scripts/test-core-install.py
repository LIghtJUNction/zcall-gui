#!/usr/bin/env python3
"""Exercise GUI bootstrap with compiled, offline curl/helper fixtures.

The fake helper models only its JSON/ownership contract. Full archive extraction,
runtime leases and rollback are tested by zcalld's real test-installer.py suite.
No request in this test reaches a network socket or a user service.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

ARGS = None
ROOT = Path(__file__).resolve().parents[1]
SOURCE = "a" * 40
VERSION = "1.2.3"

FIXTURE = r'''
#![allow(dead_code)]
#[path = JSON_MODULE] mod json;
#[path = IO_MODULE] mod io;
use json::Json;
use std::{fs, io::Write, path::{Path,PathBuf}};
const FIXTURE: &str = FIXTURE_PATH;
fn fixture()->PathBuf {PathBuf::from(FIXTURE)}
fn read(path:&Path)->Json {json::parse(&fs::read_to_string(path).unwrap()).unwrap()}
fn text(v:impl Into<String>)->Json{Json::String(v.into())}
fn config()->Json{read(&fixture().join("scenario.json"))}
fn log(name:&str){let p=fixture().join(name);let mut f=fs::OpenOptions::new().create(true).append(true).open(p).unwrap();writeln!(f,"called").unwrap();}
fn emit(value:Json){println!("{}",value.stringify());}
fn save(path:&Path,value:&Json){io::atomic(path,value.stringify().as_bytes()).unwrap();}
fn arg(args:&[String],name:&str)->String{args.windows(2).find(|p|p[0]==name).unwrap()[1].clone()}
fn outcome(plan:&Json,tx:&Path,phase:&str)->Json {
 Json::object([("ok".into(),Json::Bool(true)),("phase".into(),text(phase)),("committed".into(),Json::Bool(phase=="committed")),
 ("pending_exit".into(),Json::Bool(phase=="pending_exit")),("transaction".into(),text(tx.to_str().unwrap())),
 ("version".into(),plan.get("version").clone()),("target".into(),plan.get("target").clone()),("source".into(),plan.get("source").clone()),
 ("prefix".into(),plan.get("prefix").clone()),("binary_path".into(),text(Path::new(plan.get("prefix").text()).join("bin").join(if cfg!(windows){"zcalld.exe"}else{"zcalld"}).to_str().unwrap())),
 ("verified_source".into(),text("github-release"))])
}
fn activate(plan:&Json,tx:&Path) {
 let prefix=PathBuf::from(plan.get("prefix").text());io::directory(&prefix).unwrap();io::directory(&prefix.join("bin")).unwrap();
 let binary=prefix.join("bin").join(if cfg!(windows){"zcalld.exe"}else{"zcalld"});
 if !binary.exists(){let bytes=fs::read(std::env::current_exe().unwrap()).unwrap();let mut f=io::create(&binary).unwrap();f.write_all(&bytes).unwrap();}
 #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;fs::set_permissions(&binary,fs::Permissions::from_mode(0o700)).unwrap();}
 let receipt=Json::object([("owner".into(),text("zcalld-installer")),("schema".into(),Json::Number("1".into())),("plan".into(),plan.clone())]);
 save(&prefix.join(".zcalld-install.json"),&receipt);save(&tx.join("state.json"),&outcome(plan,tx,"committed"));
}
fn main() {
 let args=std::env::args().skip(1).collect::<Vec<_>>();
 let name=std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
 if name!="zcalld"&&name!="zcalld.exe" {
  assert!(std::env::var_os("OPENAI_API_KEY").is_none());
  assert!(std::env::var_os("ANTHROPIC_API_KEY").is_none());
 }
 if name=="curl"||name=="curl.exe" {
  log("curl.log");let url=arg(&args,"--url");
  let tail=url.strip_prefix("https://api.github.com/repos/LIghtJUNction/zcalld/").expect("only the fixed official API may be requested");
  let bytes=match tail {
   "releases/latest"=>fs::read(fixture().join("release.json")).unwrap(),
   "releases/assets/1"=>fs::read(fixture().join("helper.bin")).unwrap(),
   "releases/assets/3"=>fs::read(fixture().join("SHA256SUMS")).unwrap(),
   "git/ref/tags/v1.2.3"=>fs::read(fixture().join("reference.json")).unwrap(),
   _=>panic!("unexpected or repeated download endpoint")};
  fs::write(arg(&args,"--output"),bytes).unwrap();return;
 }
 log("helper.log");
 let config=config();let mode=config.get("mode").text();
 if name=="zcalld"||name=="zcalld.exe" {
  let home=PathBuf::from(std::env::var_os("HOME").unwrap());
  let configuration=PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap()).join("zcalld/config.conf");
  if args.iter().any(|a|a=="--config") {assert_eq!(Path::new(&arg(&args,"--config")),configuration);}
  let unit=home.join("fake-user-unit.json");let running=home.join("fake-service-running");
  let exists=fs::symlink_metadata(&configuration).is_ok();
  let configured=fs::read(&configuration).is_ok_and(|bytes|bytes==b"fixture starter\n");
  let result=match args.first().map(String::as_str) {
   Some("status")=>{
    let mut state=Json::object([("version".into(),text("1.2.3")),("healthy".into(),Json::Bool(running.exists())),
     ("configured".into(),Json::Bool(configured)),("configuration_exists".into(),Json::Bool(exists)),
     ("configuration_path".into(),text(configuration.to_str().unwrap())),
     ("service_install_supported".into(),Json::Bool(cfg!(target_os="linux"))),
     ("service_installed".into(),Json::Bool(unit.exists()))]);
    if !configured&&let Json::Object(fields)=&mut state{fields.insert("error".into(),text(if exists{"fixture invalid configuration"}else{"fixture missing configuration"}));}
    state
   },
   Some("init")=>{
    log("init.log");fs::create_dir_all(configuration.parent().unwrap()).unwrap();
    match fs::OpenOptions::new().write(true).create_new(true).open(&configuration) {
     Ok(mut f)=>{f.write_all(b"fixture starter\n").unwrap();Json::object([("ok".into(),Json::Bool(true)),("path".into(),text(configuration.to_str().unwrap()))])},
     Err(_)=>{emit(Json::object([("error".into(),text("fixture configuration already exists"))]));std::process::exit(1);}
    }
   },
   Some("service") if args.get(1).map(String::as_str)==Some("install")=>{
    log("service-install.log");assert!(configured);
    let ownership=Json::object([("binary".into(),text(std::env::current_exe().unwrap().to_str().unwrap())),("configuration".into(),text(configuration.to_str().unwrap()))]);
    let created=!unit.exists();
    if !created&&read(&unit)!=ownership{emit(Json::object([("error".into(),text("fixture unrelated unit must be preserved"))]));std::process::exit(1);}
    if created{save(&unit,&ownership);}
    Json::object([("ok".into(),Json::Bool(true)),("created".into(),Json::Bool(created)),("unit_path".into(),text(unit.to_str().unwrap())),("service_started".into(),Json::Bool(false))])
   },
   Some("service") if args.get(1).map(String::as_str)==Some("start")=>{
    log("service-start.log");assert!(configured);assert!(!cfg!(target_os="linux")||unit.exists());
    fs::write(&running,b"fake only\n").unwrap();Json::object([("ok".into(),Json::Bool(true))])
   },
   Some("providers")=>Json::object([("providers".into(),Json::Array(vec![]))]),
   Some("agents")=>Json::object([("agents".into(),Json::Array(vec![]))]),
   Some("plugins")=>Json::object([("plugins".into(),Json::Array(vec![]))]),
   Some("privacy")=>Json::object([("mode".into(),text("off"))]),
   Some("models")=>Json::object([("data".into(),Json::Array(vec![]))]),
   _=>panic!("fake installed core permits only bounded reads and explicit setup markers")};
  emit(result);return;
 }
 if args==["--version"] {println!("zcalld-installer 1.2.3");return;}
 let command=args.first().unwrap().as_str();
 if command=="defaults" {
  emit(Json::object([("ok".into(),Json::Bool(true)),("version".into(),text("1.2.3")),("target".into(),config.get("target").clone()),
   ("prefix".into(),config.get("prefix").clone()),("protocol".into(),Json::Number("1".into())),("full_archive".into(),Json::Bool(true)),("runtime_lease".into(),Json::Bool(true))]));return;
 }
 if command=="plan" {
  assert_eq!(arg(&args,"--version"),"1.2.3");let mut plan=read(&fixture().join("plan-template.json"));
  if let Json::Object(obj)=&mut plan{obj.insert("prefix".into(),text(arg(&args,"--prefix")));}
  save(Path::new(&arg(&args,"--out")),&plan);
  if let Json::Object(obj)=&mut plan {obj.insert("ok".into(),Json::Bool(true));obj.insert("phase".into(),text("planned"));}
  emit(plan);return;
 }
 if command=="apply" {
  let plan=read(Path::new(&arg(&args,"--plan")));let prefix=Path::new(plan.get("prefix").text());
  let control=prefix.parent().unwrap().join(format!(".{}.zcalld-installer",prefix.file_name().unwrap().to_str().unwrap()));
  io::directory(&control).unwrap();io::directory(&control.join("transactions")).unwrap();
  let tx=control.join("transactions").join("a".repeat(32));io::directory(&tx).unwrap();save(&tx.join("plan.json"),&plan);
  if mode=="activation_error" {
   save(&tx.join("state.json"),&outcome(&plan,&tx,"rolled_back"));
   emit(Json::object([("ok".into(),Json::Bool(false)),("error".into(),text(format!("fixture activation failure; installation journal: {}",tx.display())))]));std::process::exit(1);
  }
  if mode=="pending"||mode=="close_window" {
   let state=outcome(&plan,&tx,"pending_exit");save(&tx.join("state.json"),&state);
   if mode=="close_window" {std::thread::sleep(std::time::Duration::from_millis(700));}
   let mut result=state;if let Json::Object(obj)=&mut result{obj.insert("helper_started".into(),Json::Bool(true));}emit(result);return;
  }
  activate(&plan,&tx);emit(read(&tx.join("state.json")));return;
 }
 let tx=PathBuf::from(arg(&args,"--transaction"));let plan=read(&tx.join("plan.json"));
 match command {
  "status"=>emit(read(&tx.join("state.json"))),
  "resume"=>{activate(&plan,&tx);emit(read(&tx.join("state.json")));},
  "rollback"=>{let out=outcome(&plan,&tx,"rolled_back");save(&tx.join("state.json"),&out);emit(out);},
  _=>panic!("unexpected helper command")
 }
}
'''


def target():
    import platform
    system, machine = platform.system(), platform.machine().lower()
    cpu = "aarch64" if machine in ("arm64", "aarch64") else "x86_64"
    return {"Linux": cpu + "-unknown-linux-gnu", "Darwin": cpu + "-apple-darwin",
            "Windows": "x86_64-pc-windows-msvc"}[system]


class BootstrapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="zcall-gui-installer-")
        cls.root = Path(cls.temporary.name).resolve()
        cls.fixture = cls.root / "fixture"
        cls.fixture.mkdir()
        source = FIXTURE.replace("JSON_MODULE", json.dumps(str(ROOT / "src/json.rs"), ensure_ascii=False))
        source = source.replace("IO_MODULE", json.dumps(str(ROOT / "src/core_install_io.rs"), ensure_ascii=False))
        source = source.replace("FIXTURE_PATH", json.dumps(str(cls.fixture), ensure_ascii=False))
        file = cls.root / "fixture.rs"
        file.write_text(source, encoding="utf-8")
        cls.program = cls.root / ("fixture-bin.exe" if os.name == "nt" else "fixture-bin")
        rustc = os.environ.get("RUSTC") or shutil.which("rustc")
        if not rustc:
            raise RuntimeError("The native fixture requires the release toolchain's rustc")
        compiled = subprocess.run([rustc, "--edition=2024", str(file), "-o", str(cls.program)],
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
        if compiled.returncode:
            raise RuntimeError("Offline Rust fixture failed to compile:\n" +
                               compiled.stderr.decode("utf-8", errors="replace"))
        cls.bin = cls.root / "bin"
        cls.bin.mkdir()
        shutil.copy2(cls.program, cls.bin / ("curl.exe" if os.name == "nt" else "curl"))
        shutil.copy2(cls.program, cls.fixture / "helper.bin")

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def setUp(self):
        self.home = self.root / self._testMethodName
        self.home.mkdir(mode=0o700)
        self.env = {k: v for k, v in os.environ.items() if k in
                    {"PATH", "SystemRoot", "SYSTEMROOT", "WINDIR", "COMSPEC", "TMP", "TEMP", "TMPDIR"}}
        self.env.update(HOME=str(self.home), USERPROFILE=str(self.home),
                        XDG_CONFIG_HOME=str(self.home / "config"), XDG_DATA_HOME=str(self.home / "data"),
                        APPDATA=str(self.home / "config"), LOCALAPPDATA=str(self.home / "data"),
                        PATH=str(self.bin) + os.pathsep + self.env.get("PATH", ""),
                        OPENAI_API_KEY="fixture-must-not-reach-helper", ANTHROPIC_API_KEY="fixture-never-forward")
        if os.name == "nt":
            base = self.home / "data"
        elif __import__("sys").platform == "darwin":
            base = self.home / "Library/Application Support"
        else:
            base = self.home / "data"
        self.prefix = base / "zcalld/runtime"
        self.cache = base / "zcall-gui/core-installer"
        self.config = (self.home / "Library/Application Support" if __import__("sys").platform == "darwin" else self.home / "config") / "zcall-gui/settings.conf"
        self.mode("normal")
        for name in ["helper.log", "curl.log", "init.log", "service-install.log", "service-start.log"]:
            (self.fixture / name).unlink(missing_ok=True)
        helper_name = f"zcalld-installer-{VERSION}-{target()}" + (".exe" if os.name == "nt" else "")
        archive_name = f"zcalld-{VERSION}-{target()}" + (".zip" if os.name == "nt" else ".tar.gz")
        digest = hashlib.sha256(self.program.read_bytes()).hexdigest()
        self.release = {"id": 42, "tag_name": "v" + VERSION, "draft": False, "prerelease": False,
                        "assets": [{"name": helper_name, "id": 1, "size": self.program.stat().st_size},
                                   {"name": archive_name, "id": 2, "size": 100},
                                   {"name": "SHA256SUMS", "id": 3, "size": 300}]}
        self.write("release.json", self.release)
        self.write("reference.json", {"ref": "refs/tags/v1.2.3", "object": {"type": "commit", "sha": SOURCE}})
        (self.fixture / "SHA256SUMS").write_text(f"{digest}  {helper_name}\n{'b' * 64}  {archive_name}\n", encoding="utf-8")
        self.write("plan-template.json", {"version": VERSION, "target": target(), "source": SOURCE,
                    "prefix": str(self.prefix), "archive_name": archive_name, "archive_sha256": "b" * 64,
                    "archive_asset_id": 2, "release_id": 42, "created_at_ms": 1234})

    def mode(self, value):
        self.write("scenario.json", {"mode": value, "target": target(), "prefix": str(self.prefix)})

    def write(self, name, value):
        (self.fixture / name).write_text(json.dumps(value), encoding="utf-8")

    def run_gui(self, command, success=True, binary="missing-fixture-zcalld"):
        result = subprocess.run([ARGS.binary, "--zcalld", binary, "--core-install", command], env=self.env,
                                capture_output=True, text=True, timeout=60)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("fixture-must-not-reach-helper", result.stdout + result.stderr)
        return json.loads(result.stdout) if result.stdout.strip() else None

    def saved(self):
        return json.loads((self.cache / "latest.json").read_text(encoding="utf-8"))

    def calls(self, name):
        path = self.fixture / name
        return len(path.read_text().splitlines()) if path.exists() else 0

    def test_first_install_verifies_plan_and_remembers_actual_binary(self):
        plan = self.run_gui("prepare")
        self.assertEqual(plan["phase"], "planned")
        self.assertFalse(self.prefix.exists())
        done = self.run_gui("apply")
        self.assertEqual(done["phase"], "committed")
        self.assertTrue(done["committed"])
        self.assertIn("core_binary=" + done["binary_path"], self.config.read_text(encoding="utf-8"))
        self.assertEqual(self.run_gui("status")["binary_path"], done["binary_path"])
        self.assertEqual(self.calls("curl.log"), 4, "No network check is part of apply/status")

    def test_pending_exit_requires_explicit_completion(self):
        self.mode("pending")
        self.run_gui("prepare")
        pending = self.run_gui("apply")
        self.assertEqual(pending["phase"], "pending_exit")
        self.assertTrue(pending["helper_started"])
        self.assertFalse(pending["committed"])
        self.assertFalse(self.config.exists())
        self.assertFalse(self.prefix.exists())
        self.assertEqual(self.run_gui("status")["phase"], "pending_exit")
        self.assertTrue(self.run_gui("resume")["committed"])

    def test_closing_window_preserves_helper_transaction_for_reopen(self):
        self.mode("close_window")
        self.run_gui("prepare")
        process = subprocess.Popen([ARGS.binary, "--core-install", "apply"], env=self.env,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        tx = self.prefix.parent / ".runtime.zcalld-installer/transactions" / ("a" * 32)
        deadline = time.monotonic() + 45
        while not (tx / "state.json").exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertTrue((tx / "state.json").exists())
        process.kill()  # Only this isolated test GUI; its helper is allowed to finish.
        process.wait(timeout=5)
        self.assertEqual(self.run_gui("status")["phase"], "pending_exit")
        time.sleep(0.8)
        self.assertEqual(self.run_gui("status")["phase"], "pending_exit")
        self.assertTrue(self.run_gui("resume")["committed"])

    def test_activation_failure_keeps_bound_recovery_journal(self):
        self.mode("activation_error")
        self.run_gui("prepare")
        failed = self.run_gui("apply", success=False)
        self.assertFalse(failed["ok"])
        self.assertEqual(self.run_gui("status")["phase"], "rolled_back")
        self.assertFalse(self.config.exists())

    def test_wrong_helper_hash_executes_nothing(self):
        checksum = self.fixture / "SHA256SUMS"
        text = checksum.read_text(encoding="utf-8")
        checksum.write_text("f" * 64 + text[64:], encoding="utf-8")
        self.run_gui("prepare", success=False)
        self.assertEqual(self.calls("helper.log"), 0)
        self.assertFalse(self.prefix.exists())

    def test_missing_bundle_or_changed_tag_is_rejected_before_helper(self):
        self.release["assets"] = self.release["assets"][:1]
        self.write("release.json", self.release)
        self.run_gui("prepare", success=False)
        self.assertEqual(self.calls("helper.log"), 0)

    def test_approved_plan_change_blocks_apply(self):
        self.run_gui("prepare")
        path = Path(self.saved()["directory"]) / "plan.json"
        with path.open("a", encoding="utf-8") as output:
            output.write(" ")
        calls = self.calls("helper.log")
        self.run_gui("apply", success=False)
        self.assertEqual(self.calls("helper.log"), calls)
        self.assertFalse(self.prefix.exists())

    def test_changed_downloaded_helper_blocks_status(self):
        self.run_gui("prepare")
        path = Path(self.saved()["directory"]) / ("zcalld-installer.exe" if os.name == "nt" else "zcalld-installer")
        with path.open("ab") as output:
            output.write(b"changed")
        self.run_gui("status", success=False)
        self.assertFalse(self.prefix.exists())

    def test_package_managed_binary_never_uses_native_bootstrap(self):
        managed = Path(os.environ["SystemRoot"]) / "System32/cmd.exe" if os.name == "nt" else Path("/usr/bin/true")
        self.assertTrue(managed.is_file())
        self.run_gui("prepare", success=False, binary=str(managed))
        self.assertEqual(self.calls("curl.log"), 0)
        self.assertFalse(self.prefix.exists())


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default=os.environ.get("ZCALL_GUI_TEST_BINARY", str(ROOT / "target/release/zcall-gui")))
    ARGS, remaining = parser.parse_known_args()
    ARGS.binary = str(Path(ARGS.binary).resolve(strict=True))
    unittest.main(argv=[__file__] + remaining)
