#!/usr/bin/env python3
"""Publish one immutable native release; resume matching partial uploads."""
import argparse,hashlib,json,pathlib,subprocess,tempfile
from release import ROOT,current,exact,validate
from package import TARGETS
REPO="LIghtJUNction/zcall-gui"
def gh(*args,check=True):return subprocess.run(["gh",*args],cwd=ROOT,text=True,capture_output=True,check=check)
def expected(version):
    names={f"zcall-gui-v{version}-source.tar.gz"}
    for target in TARGETS:
        stem=f"zcall-gui-v{version}-{target}";raw=stem+(".exe" if "windows" in target else "")
        names.update([raw,raw+".sha256",stem+(".zip" if "windows" in target else ".tar.gz")])
    return names
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def publish(tag):
    exact(tag);version=current();dist=ROOT/"_release"/"dist";names=expected(version)
    present={p.name for p in dist.iterdir() if p.is_file() and p.name!="SHA256SUMS"}
    if present!=names:raise ValueError(f"Release asset set differs: missing={sorted(names-present)}, extra={sorted(present-names)}")
    for name in sorted(names):
        if name.endswith(".sha256"):
            raw=name.removesuffix(".sha256")
            if (dist/name).read_text().strip()!=f"{digest(dist/raw)}  {raw}":raise ValueError("Raw binary checksum file disagrees")
    sums=dist/"SHA256SUMS";sums.write_text("".join(f"{digest(dist/name)}  {name}\n" for name in sorted(names)));names.add(sums.name)
    existing=gh("release","view",tag,"--repo",REPO,"--json","tagName,isDraft,assets",check=False)
    latest=gh("api",f"repos/{REPO}/releases/latest",check=False);make_latest=True
    if latest.returncode==0:
        latest_tag=json.loads(latest.stdout).get("tag_name","")
        make_latest=validate(version)>=validate(latest_tag.removeprefix("v"))
    if existing.returncode:
        notes=ROOT/"_release"/"release-notes.md"
        notes.write_text(f"# zcall-gui {version}\n\nNative Project Z control panel for zcalld 0.2 or later. All used Z sources are pinned Git submodules.\n\n- Linux x86_64/aarch64, Windows x86_64, macOS Intel/Apple Silicon.\n- SHA-256 verified self-updates for user installations; AUR installations use the package manager.\n- Linux uses X11/XWayland and OpenGL. macOS uses the deprecated system OpenGL compatibility backend.\n- No third-party Rust crates. Native graphics drivers, fonts and platform libraries remain runtime dependencies.\n\nThe source.tar.gz asset includes zui; GitHub's automatic source archives do not include submodule contents.\n\nSee README and docs/VALIDATION.md for measured scope and limitations. Binaries are not platform-signed.\n")
        gh("release","create",tag,"--repo",REPO,"--verify-tag","--draft","--title",tag,"--notes-file",str(notes),"--latest="+str(make_latest).lower())
        info={"isDraft":True,"assets":[]}
    else:info=json.loads(existing.stdout)
    remote={asset["name"] for asset in info["assets"]}
    if remote-names:raise ValueError("Existing release contains unexpected assets; refusing to change it")
    if remote:
        with tempfile.TemporaryDirectory(prefix="zcall-gui-release-compare-") as directory:
            gh("release","download",tag,"--repo",REPO,"--dir",directory)
            for name in remote:
                if digest(pathlib.Path(directory)/name)!=digest(dist/name):raise ValueError(f"Published asset is immutable and differs: {name}")
    missing=names-remote
    if missing and not info["isDraft"]:raise ValueError("Published release is incomplete; refusing to mutate public assets")
    if missing:gh("release","upload",tag,*[str(dist/name) for name in sorted(missing)],"--repo",REPO)
    if info["isDraft"]:gh("release","edit",tag,"--repo",REPO,"--draft=false","--latest="+str(make_latest).lower())
    print(f"https://github.com/{REPO}/releases/tag/{tag}")
def main():
    parser=argparse.ArgumentParser();parser.add_argument("tag");args=parser.parse_args()
    try:publish(args.tag)
    except (ValueError,subprocess.CalledProcessError)as error:raise SystemExit(str(error))
if __name__=="__main__":main()
