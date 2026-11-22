#!/usr/bin/env python3
"""Create native binary archives and a source archive that includes submodules."""
import argparse,gzip,hashlib,io,pathlib,plistlib,shutil,subprocess,tarfile,tempfile,zipfile
from release import ROOT,current,clean
TARGETS={"x86_64-unknown-linux-gnu","aarch64-unknown-linux-gnu","x86_64-pc-windows-msvc","aarch64-apple-darwin","x86_64-apple-darwin"}
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def checksum(path):path.with_name(path.name+".sha256").write_text(f"{digest(path)}  {path.name}\n")
def copy(source,target,executable=False):
    target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target);target.chmod(0o755 if executable else 0o644)
def tar(source,destination):
    epoch=int(subprocess.check_output(["git","show","-s","--format=%ct","HEAD"],cwd=ROOT,text=True).strip())
    def normalize(info):
        info.uid=info.gid=0;info.uname=info.gname="root";info.mtime=epoch
        if info.isfile():info.mode=0o755 if info.mode&0o111 else 0o644
        return info
    with destination.open("wb") as output,gzip.GzipFile(filename="",mode="wb",fileobj=output,mtime=epoch) as compressed,tarfile.open(fileobj=compressed,mode="w") as archive:
        archive.add(source,arcname=source.name,filter=normalize)
def package(target):
    if target not in TARGETS:raise ValueError("Unknown native target")
    host=next(line.split(": ",1)[1] for line in subprocess.check_output(["rustc","-vV"],text=True).splitlines() if line.startswith("host: "))
    if host!=target:raise ValueError("Package target must match the native build host")
    version=current();dist=ROOT/"_release"/"dist";dist.mkdir(parents=True,exist_ok=True)
    ext=".exe" if "windows" in target else "";binary=ROOT/"target"/"release"/("zcall-gui"+ext)
    if not binary.is_file():raise ValueError("Build the native release binary first")
    if subprocess.check_output([str(binary),"--version"],text=True).strip()!=f"zcall-gui {version}":raise ValueError("Binary version does not match Cargo.toml")
    raw=dist/f"zcall-gui-v{version}-{target}{ext}";copy(binary,raw,True);checksum(raw)
    with tempfile.TemporaryDirectory(prefix="zcall-gui-package-") as directory:
        tree=pathlib.Path(directory)/f"zcall-gui-v{version}-{target}";tree.mkdir()
        copy(ROOT/"README.md",tree/"README.md");copy(ROOT/"LICENSE",tree/"LICENSE")
        shutil.copytree(ROOT/"docs",tree/"docs")
        if "windows" in target:
            copy(binary,tree/"zcall-gui.exe",True);copy(ROOT/"vendor"/"zgpui"/"LICENSE",tree/"zgpui-LICENSE")
            archive=dist/(tree.name+".zip")
            with zipfile.ZipFile(archive,"w",zipfile.ZIP_DEFLATED) as z:
                for file in sorted(tree.rglob("*")):
                    if file.is_file():z.write(file,file.relative_to(tree.parent))
        elif "apple" in target:
            app=tree/"zcall-gui.app"/"Contents";copy(binary,app/"MacOS"/"zcall-gui",True)
            copy(ROOT/"packaging"/"zcall-gui.icns",app/"Resources"/"zcall-gui.icns")
            (app/"Info.plist").write_bytes(plistlib.dumps({"CFBundleIconFile":"zcall-gui.icns","CFBundleName":"zcall-gui","CFBundleDisplayName":"zcall GUI","CFBundleIdentifier":"org.project-z.zcall-gui","CFBundleExecutable":"zcall-gui","CFBundlePackageType":"APPL","CFBundleShortVersionString":version,"CFBundleVersion":version,"LSMinimumSystemVersion":"13.0","NSPrincipalClass":"NSApplication","NSHighResolutionCapable":True}))
            copy(ROOT/"vendor"/"zgpui"/"LICENSE",tree/"zgpui-LICENSE");archive=dist/(tree.name+".tar.gz");tar(tree,archive)
        else:
            copy(binary,tree/"bin"/"zcall-gui",True)
            for source,target_file in [
                (ROOT/"packaging"/"zcall-gui.desktop",tree/"share"/"applications"/"zcall-gui.desktop"),
                (ROOT/"packaging"/"zcall-gui.svg",tree/"share"/"icons"/"hicolor"/"scalable"/"apps"/"zcall-gui.svg"),
                (ROOT/"LICENSE",tree/"share"/"licenses"/"zcall-gui"/"LICENSE"),
                (ROOT/"vendor"/"zgpui"/"LICENSE",tree/"share"/"licenses"/"zcall-gui"/"zgpui-LICENSE")]:copy(source,target_file)
            (tree/"install.sh").write_text("""#!/bin/sh
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=${PREFIX:-"$HOME/.local"}
dest=${DESTDIR:-}
install -Dm755 "$base/bin/zcall-gui" "$dest$prefix/bin/zcall-gui"
install -Dm644 "$base/share/applications/zcall-gui.desktop" "$dest$prefix/share/applications/zcall-gui.desktop"
install -Dm644 "$base/share/icons/hicolor/scalable/apps/zcall-gui.svg" "$dest$prefix/share/icons/hicolor/scalable/apps/zcall-gui.svg"
install -Dm644 "$base/share/licenses/zcall-gui/LICENSE" "$dest$prefix/share/licenses/zcall-gui/LICENSE"
install -Dm644 "$base/share/licenses/zcall-gui/zgpui-LICENSE" "$dest$prefix/share/licenses/zcall-gui/zgpui-LICENSE"
install -d "$dest$prefix/share/doc/zcall-gui"
cp -R "$base/README.md" "$base/docs" "$dest$prefix/share/doc/zcall-gui/"
printf 'Installed zcall-gui to %s/bin\\n' "$prefix"
""");(tree/"install.sh").chmod(0o755);archive=dist/(tree.name+".tar.gz");tar(tree,archive)
    print(raw);print(archive)
def source():
    clean();dist=ROOT/"_release"/"dist";dist.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="zcall-gui-source-") as directory:
        tree=pathlib.Path(directory)/("zcall-gui-v"+current());tree.mkdir()
        for repository,prefix in [(ROOT,""),(ROOT/"vendor"/"zgpui","vendor/zgpui/")]:
            data=subprocess.check_output(["git","archive","--format=tar","--prefix="+prefix,"HEAD"],cwd=repository)
            with tarfile.open(fileobj=io.BytesIO(data),mode="r:") as archive:archive.extractall(tree,filter="data")
        # The vendored workspace has optional and test dependencies of its own.
        # Cargo emits a relocatable source replacement when given a relative path.
        config = subprocess.check_output(
            ["cargo", "vendor", "--locked", "--manifest-path", "vendor/zgpui/Cargo.toml",
             "vendor/cargo-sources"], cwd=tree, text=True)
        (tree / ".cargo").mkdir(exist_ok=True)
        (tree / ".cargo/config.toml").write_text(config, encoding="utf-8")
        destination=dist/("zcall-gui-v"+current()+"-source.tar.gz");tar(tree,destination);print(destination)
def main():
    parser=argparse.ArgumentParser();parser.add_argument("--target",choices=sorted(TARGETS));parser.add_argument("--source",action="store_true");args=parser.parse_args()
    if args.source:source()
    elif args.target:package(args.target)
    else:parser.error("choose --target or --source")
if __name__=="__main__":main()
