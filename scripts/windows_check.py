#!/usr/bin/env python3
"""Select the installed MSVC environment without printing environment values."""
import os,pathlib,subprocess,sys
root=pathlib.Path(__file__).resolve().parents[1]
vswhere=pathlib.Path(os.environ["ProgramFiles(x86)"])/"Microsoft Visual Studio"/"Installer"/"vswhere.exe"
installation=subprocess.check_output([str(vswhere),"-latest","-products","*","-requires","Microsoft.VisualStudio.Component.VC.Tools.x86.x64","-property","installationPath"],text=True).strip()
if not installation:raise SystemExit("MSVC x64 build tools are required")
batch=pathlib.Path(installation)/"VC"/"Auxiliary"/"Build"/"vcvars64.bat"
output=subprocess.check_output(["cmd","/d","/s","/c",f'""{batch}" >nul && set"'],text=True)
environment=os.environ.copy()
for line in output.splitlines():
    if "=" in line and not line.startswith("="):
        key,value=line.split("=",1);environment[key]=value
subprocess.run([sys.executable,"scripts/check.py"],cwd=root,env=environment,check=True)
subprocess.run([str(root/"target"/"release"/"zcall-gui.exe"),"--smoke-seconds","2","--zcalld","zcalld-release-fixture-not-installed"],cwd=root,env=environment,check=True)
