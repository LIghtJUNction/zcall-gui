#!/usr/bin/env python3
import json,os,pathlib
import release
kind=os.environ.get("BUMP_KIND","patch")
dry=os.environ.get("DRY_RUN","false")=="true"
result=release.bump(kind,os.environ.get("CUSTOM_VERSION") or None,dry)
tag=result["tag"]
if not dry:
    release.run("git","config","user.name","github-actions[bot]")
    release.run("git","config","user.email","41898282+github-actions[bot]@users.noreply.github.com")
    release.run("git","add","Cargo.toml","Cargo.lock","CHANGELOG.md")
    release.run("git","-c","commit.gpgsign=false","commit","-m",f"chore: release {tag}")
    release.run("git","-c","tag.gpgsign=false","tag","-a",tag,"-m",f"Release {tag}")
    release.run("git","push","--atomic","origin","HEAD:main","refs/tags/"+tag)
with pathlib.Path(os.environ["GITHUB_OUTPUT"]).open("a")as output:
    output.write(f"tag={tag}\npublish={str(not dry).lower()}\n")
print(json.dumps(result))
