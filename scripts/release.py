#!/usr/bin/env python3
"""Version transactions and exact release-ref validation using Python stdlib."""
import argparse,json,os,pathlib,re,subprocess,sys,tomllib
ROOT=pathlib.Path(__file__).resolve().parents[1]
VERSION=re.compile(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)\Z")
def run(*args,check=True,capture=False):
    return subprocess.run(args,cwd=ROOT,check=check,text=True,capture_output=capture)
def current():
    return tomllib.loads((ROOT/"Cargo.toml").read_text())["package"]["version"]
def validate(version):
    if not VERSION.fullmatch(version):raise ValueError("Use a stable major.minor.patch version")
    return tuple(map(int,version.split(".")))
def clean():
    if run("git","status","--porcelain",capture=True).stdout:raise ValueError("Commit the worktree and submodule changes before a release")
    lines=run("git","submodule","status","--recursive",capture=True).stdout.splitlines()
    if any(line.startswith(("-","+","U")) for line in lines):raise ValueError("Submodules must be initialized at their committed revisions")
def exact(tag):
    if tag!="v"+current():raise ValueError("Release tag does not match Cargo.toml")
    validate(tag[1:])
    head=run("git","rev-parse","HEAD",capture=True).stdout.strip()
    tagged=run("git","rev-parse","--verify",f"refs/tags/{tag}^{{commit}}",capture=True).stdout.strip()
    if tagged!=head:raise ValueError("Checkout is not the requested immutable tag")
    clean()
    return tag
def candidate(commit):
    if not re.fullmatch(r"[0-9a-f]{40}",commit):raise ValueError("Candidate ref must be a full commit SHA")
    head=run("git","rev-parse","HEAD",capture=True).stdout.strip()
    if head!=commit:raise ValueError("Candidate checkout does not match the requested commit")
    clean()
    return commit
def bump(kind,custom=None,dry_run=False):
    old=current();parts=list(validate(old))
    if kind=="custom":
        if not custom:raise ValueError("custom requires --version")
        new=custom
    else:
        index={"major":0,"minor":1,"patch":2}[kind];parts[index]+=1
        for at in range(index+1,3):parts[at]=0
        new=".".join(map(str,parts))
    if validate(new)<=validate(old):raise ValueError("New version must be greater than the current version")
    tag="v"+new;clean()
    if run("git","show-ref","--verify","--quiet","refs/tags/"+tag,check=False).returncode==0:raise ValueError("Release tag already exists")
    if dry_run:return {"old":old,"new":new,"tag":tag,"dry_run":True}
    paths=[ROOT/"Cargo.toml",ROOT/"Cargo.lock",ROOT/"CHANGELOG.md"];before={p:p.read_bytes() for p in paths}
    try:
        cargo=paths[0].read_text();needle=f'version = "{old}"'
        if cargo.count(needle)!=1:raise ValueError("Package version is ambiguous")
        paths[0].write_text(cargo.replace(needle,f'version = "{new}"',1))
        run("cargo","update","--workspace","--offline")
        changelog=paths[2].read_text()
        if "## Unreleased" in changelog:changelog=changelog.replace("## Unreleased",f"## {new}",1)
        else:
            previous=run("git","tag","--list",f"v{old}",capture=True).stdout.strip()
            history=run("git","log","--format=- %s",f"v{old}..HEAD" if previous else "HEAD",capture=True).stdout.strip()
            changelog=changelog.replace("# Changelog",f"# Changelog\n\n## {new}\n\n"+(history or "- Release maintenance."),1)
        paths[2].write_text(changelog)
        lock=tomllib.loads(paths[1].read_text());match=[p for p in lock["package"] if p["name"]=="zcall-gui"]
        if len(match)!=1 or match[0]["version"]!=new:raise ValueError("Cargo.lock did not update the application version")
    except BaseException:
        for path,data in before.items():path.write_bytes(data)
        raise
    return {"old":old,"new":new,"tag":tag,"dry_run":False}
def main():
    parser=argparse.ArgumentParser();sub=parser.add_subparsers(dest="command",required=True)
    b=sub.add_parser("bump");b.add_argument("kind",choices=["patch","minor","major","custom"]);b.add_argument("--version");b.add_argument("--dry-run",action="store_true")
    v=sub.add_parser("verify-tag");v.add_argument("tag")
    c=sub.add_parser("verify-candidate");c.add_argument("commit")
    args=parser.parse_args()
    try:
        result=bump(args.kind,args.version,args.dry_run) if args.command=="bump" else ({"tag":exact(args.tag)} if args.command=="verify-tag" else {"commit":candidate(args.commit)})
        print(json.dumps(result))
    except (ValueError,subprocess.CalledProcessError) as error:raise SystemExit(str(error))
if __name__=="__main__":main()
