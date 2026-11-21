#!/usr/bin/env python3
"""Local gate; no GitHub workflows are triggered by this script."""
import json,os,pathlib,subprocess,sys
root=pathlib.Path(__file__).resolve().parents[1]
def target_directory(manifest=None):
    command=["cargo","metadata","--no-deps","--format-version","1","--locked","--offline"]
    if manifest is not None:command.extend(["--manifest-path",manifest])
    return pathlib.Path(json.loads(subprocess.check_output(command,cwd=root,text=True))["target_directory"])
commands=[
 ["cargo","fmt","--check"],
 ["cargo","fmt","--manifest-path","vendor/zgpui/Cargo.toml","--check"],
 ["cargo","clippy","--locked","--offline","--all-targets","--","-D","warnings"],
 ["cargo","clippy","--manifest-path","vendor/zgpui/Cargo.toml","--features","native","--locked","--offline","--all-targets","--","-D","warnings"],
 ["cargo","test","--locked","--offline"],
 ["cargo","test","--manifest-path","vendor/zgpui/Cargo.toml","--features","native","--locked","--offline"],
 ["python3" if os.name!="nt" else "python","scripts/test_release.py"],
 ["cargo","build","--release","--locked","--offline"],
]
for command in commands:
    print("+"," ".join(command),flush=True)
    subprocess.run(command,cwd=root,check=True)
binary=target_directory()/"release"/("zcall-gui.exe" if os.name=="nt" else "zcall-gui")
subprocess.run([sys.executable,"scripts/test-core-install.py","--binary",str(binary.resolve())],cwd=root,check=True)
if "--ui" in sys.argv:
    if sys.platform!="linux":raise SystemExit("--ui requires an isolated Linux Xvfb display")
    subprocess.run(["cargo","build","--manifest-path","vendor/zgpui/Cargo.toml","--features","native","--release","--example","window_probe","--locked","--offline"],cwd=root,check=True)
    subprocess.run(["xvfb-run","-a","-s","-screen 0 1280x900x24 -nolisten tcp","python3","vendor/zgpui/scripts/check_x11.py","--binary",str(target_directory("vendor/zgpui/Cargo.toml")/"release/examples/window_probe"),"--output","_validation/x11-properties.json"],cwd=root,check=True)
    for script in ["scripts/ui_smoke.py", "scripts/usage_recording_smoke.py", "scripts/managed_update_smoke.py", "scripts/core_install_ui_smoke.py"]:
        subprocess.run(["xvfb-run","-a","-s","-screen 0 1100x740x24 -nolisten tcp","python3",script,str(binary.resolve())],cwd=root,check=True)
