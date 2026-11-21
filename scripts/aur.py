#!/usr/bin/env python3
"""Render AUR metadata from public release bytes; optionally publish it."""
import argparse,hashlib,pathlib,re,shutil,subprocess,urllib.request
from release import ROOT,current,validate
BASE="https://github.com/LIghtJUNction/zcall-gui"
DEPS=["zcalld>=0.2.0","glibc","libgcc","libx11","libgl","freetype2","fontconfig","curl","hicolor-icon-theme"]
def download_hash(name,version,directory):
    url=f"{BASE}/releases/download/v{version}/{name}"
    path=directory/name;directory.mkdir(parents=True,exist_ok=True)
    with urllib.request.urlopen(urllib.request.Request(url,headers={"User-Agent":"zcall-gui-release"}),timeout=90) as response,path.open("wb") as output:shutil.copyfileobj(response,output)
    return hashlib.sha256(path.read_bytes()).hexdigest()
def positive_revision(value):
    if isinstance(value,bool) or not re.fullmatch(r"[1-9][0-9]*",str(value)):raise ValueError("Source package revision must be a positive integer")
    return int(value)
def package_names(source_only=False):return ["zcall-gui"] if source_only else ["zcall-gui","zcall-gui-bin"]
def metadata(name,version,hashes,source_pkgrel=1):
    source_pkgrel=positive_revision(source_pkgrel)
    binary=name.endswith("-bin");description="Native Project Z control panel for the zcalld local AI gateway"+(" (binary)" if binary else "")
    lines=[f"pkgbase = {name}",f"\tpkgdesc = {description}",f"\tpkgver = {version}",f"\tpkgrel = {1 if binary else source_pkgrel}",f"\turl = {BASE}","\tarch = x86_64","\tarch = aarch64","\tlicense = MIT"]
    if not binary:lines.extend(["\tmakedepends = rust","\tmakedepends = pkgconf"])
    lines.extend("\tdepends = "+dependency for dependency in DEPS)
    lines.extend(["\toptdepends = xorg-xwayland: run on Wayland sessions","\toptdepends = noto-fonts-cjk: Chinese and Japanese glyphs"])
    if binary:
        lines.extend([f"\tprovides = zcall-gui={version}","\tconflicts = zcall-gui","\toptions = !strip"])
        for arch in ["x86_64","aarch64"]:
            filename=f"zcall-gui-v{version}-{arch}-unknown-linux-gnu.tar.gz"
            lines.extend([f"\tsource_{arch} = {filename}::{BASE}/releases/download/v{version}/{filename}",f"\tsha256sums_{arch} = {hashes[arch]}"])
    else:
        lines.append("\toptions = !lto")
        filename=f"zcall-gui-v{version}-source.tar.gz"
        lines.extend([f"\tsource = {filename}::{BASE}/releases/download/v{version}/{filename}",f"\tsha256sums = {hashes['source']}"])
    return "\n".join(lines)+f"\n\npkgname = {name}\n"
def package_version(text):
    fields=dict(re.findall(r"^\s*(epoch|pkgver|pkgrel)\s*=\s*(\S+)\s*$",text,re.M))
    if "pkgver" not in fields or "pkgrel" not in fields:raise ValueError("Existing AUR metadata has no version")
    return (int(fields.get("epoch","0")),validate(fields["pkgver"]),int(fields["pkgrel"]))
def render_recipe(name,version,hashes,source_pkgrel=1):
    validate(version);source_pkgrel=positive_revision(source_pkgrel)
    if name not in package_names():raise ValueError("Unknown AUR package")
    recipe=(ROOT/"packaging"/"aur"/name/"PKGBUILD.in").read_text()
    replacements={"VERSION":version,**{key.upper()+"_SHA256":value for key,value in hashes.items()}}
    for key,value in replacements.items():recipe=recipe.replace("@"+key+"@",value)
    if re.search(r"@[A-Z0-9_]+@",recipe):raise ValueError("Unresolved recipe placeholder")
    if name=="zcall-gui":
        recipe,count=re.subn(r"^pkgrel=1$",f"pkgrel={source_pkgrel}",recipe,flags=re.M)
        if count!=1:raise ValueError("Source template must have exactly one default package revision")
    return recipe
def render(version,source_pkgrel=1,source_only=False):
    validate(version);source_pkgrel=positive_revision(source_pkgrel);inputs=ROOT/"_release"/"aur-inputs"
    names={"source":f"zcall-gui-v{version}-source.tar.gz","x86_64":f"zcall-gui-v{version}-x86_64-unknown-linux-gnu.tar.gz","aarch64":f"zcall-gui-v{version}-aarch64-unknown-linux-gnu.tar.gz"}
    if source_only:names={"source":names["source"]}
    hashes={key:download_hash(filename,version,inputs) for key,filename in names.items()}
    generated=ROOT/"_release"/"aur-generated"
    for name in package_names(source_only):
        recipe=render_recipe(name,version,hashes,source_pkgrel)
        path=generated/name;path.mkdir(parents=True,exist_ok=True);(path/"PKGBUILD").write_text(recipe);(path/".SRCINFO").write_text(metadata(name,version,hashes,source_pkgrel))
    return generated
def publish(generated,source_only=False):
    for name in package_names(source_only):
        checkout=ROOT/"_release"/"aur"/name
        if checkout.exists():raise ValueError(f"Use a fresh AUR checkout; already exists: {checkout}")
        checkout.parent.mkdir(parents=True,exist_ok=True)
        subprocess.run(["git","clone",f"ssh://aur@aur.archlinux.org/{name}.git",str(checkout)],check=True)
        incoming=(generated/name/".SRCINFO").read_text();old=checkout/".SRCINFO"
        if old.exists() and package_version(old.read_text())>package_version(incoming):
            print(f"Skipped {name}: AUR already has a newer version or package revision");continue
        for file in ["PKGBUILD",".SRCINFO"]:shutil.copyfile(generated/name/file,checkout/file)
        def git(*args,**kwargs):return subprocess.run(["git",*args],cwd=checkout,check=True,**kwargs)
        git("config","user.name","LIghtJUNction");git("config","user.email","106986785+LIghtJUNction@users.noreply.github.com")
        git("add","PKGBUILD",".SRCINFO")
        if subprocess.run(["git","diff","--cached","--quiet"],cwd=checkout).returncode==0:print(f"{name} is already current");continue
        incoming_fields=dict(re.findall(r"^\s*(pkgver|pkgrel)\s*=\s*(\S+)\s*$",incoming,re.M))
        git("-c","commit.gpgsign=false","commit","-m",f"Update {name} to {incoming_fields['pkgver']}-{incoming_fields['pkgrel']}")
        git("push","origin","HEAD:master")
def main():
    parser=argparse.ArgumentParser();parser.add_argument("--version",default=current());parser.add_argument("--source-pkgrel",type=positive_revision,default=1);parser.add_argument("--source-only",action="store_true");parser.add_argument("--publish",action="store_true");args=parser.parse_args()
    generated=render(args.version,args.source_pkgrel,args.source_only)
    if args.publish:publish(generated,args.source_only)
    print(generated)
if __name__=="__main__":main()
