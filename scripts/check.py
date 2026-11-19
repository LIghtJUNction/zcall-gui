#!/usr/bin/env python3
"""Local gate; no GitHub workflows are triggered by this script."""
import os,pathlib,subprocess,sys
root=pathlib.Path(__file__).resolve().parents[1]
commands=[
 ["cargo","fmt","--check"],
 ["cargo","fmt","--manifest-path","vendor/zui/Cargo.toml","--check"],
 ["cargo","clippy","--locked","--offline","--all-targets","--","-D","warnings"],
 ["cargo","clippy","--manifest-path","vendor/zui/Cargo.toml","--locked","--offline","--all-targets","--","-D","warnings"],
 ["cargo","test","--locked","--offline"],
 ["cargo","test","--manifest-path","vendor/zui/Cargo.toml","--locked","--offline"],
 ["python3" if os.name!="nt" else "python","scripts/test_release.py"],
 ["cargo","build","--release","--locked","--offline"],
]
for command in commands:
    print("+"," ".join(command),flush=True)
    subprocess.run(command,cwd=root,check=True)
if "--ui" in sys.argv:
    if sys.platform!="linux":raise SystemExit("--ui requires an isolated Linux Xvfb display")
    subprocess.run(["cargo","build","--manifest-path","vendor/zui/Cargo.toml","--release","--example","window_probe","--locked","--offline"],cwd=root,check=True)
    subprocess.run(["xvfb-run","-a","-s","-screen 0 1280x900x24 -nolisten tcp","python3","vendor/zui/scripts/check_x11.py","--binary","vendor/zui/target/release/examples/window_probe","--output","_validation/x11-properties.json"],cwd=root,check=True)
    for script in ["scripts/ui_smoke.py", "scripts/managed_update_smoke.py"]:
        subprocess.run(["xvfb-run","-a","-s","-screen 0 1100x740x24 -nolisten tcp","python3",script],cwd=root,check=True)
