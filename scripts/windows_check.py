#!/usr/bin/env python3
"""Select the installed MSVC environment without printing environment values."""
import os,pathlib,subprocess,sys,tempfile
root=pathlib.Path(__file__).resolve().parents[1]
vswhere=pathlib.Path(os.environ["ProgramFiles(x86)"])/"Microsoft Visual Studio"/"Installer"/"vswhere.exe"
installation=subprocess.check_output([str(vswhere),"-latest","-products","*","-requires","Microsoft.VisualStudio.Component.VC.Tools.x86.x64","-property","installationPath"],text=True).strip()
if not installation:raise SystemExit("MSVC x64 build tools are required")
batch=pathlib.Path(installation)/"VC"/"Auxiliary"/"Build"/"vcvars64.bat"
with tempfile.TemporaryDirectory(prefix="zcall-msvc-") as directory:
    script=pathlib.Path(directory)/"environment.cmd"
    script.write_text('@echo off\nchcp 65001 >nul\ncall "'+str(batch)+'" >nul\nif errorlevel 1 exit /b %errorlevel%\nset\n',encoding="utf-8")
    output=subprocess.check_output(["cmd","/d","/c",script.name],cwd=directory,text=True,encoding="utf-8")
environment={key.upper():value for key,value in os.environ.items()}
for line in output.splitlines():
    if "=" in line and not line.startswith("="):
        key,value=line.split("=",1);environment[key.upper()]=value
subprocess.run([sys.executable,"scripts/check.py"],cwd=root,env=environment,check=True)
subprocess.run([str(root/"target"/"release"/"zcall-gui.exe"),"--smoke-seconds","2","--zcalld","zcalld-release-fixture-not-installed"],cwd=root,env=environment,check=True)
