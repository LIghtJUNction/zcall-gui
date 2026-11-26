#!/usr/bin/env python3
"""Exercise only managed-update UI wiring with private, inert package tools."""
import argparse, json, os, pathlib, subprocess, sys, tempfile, time
from PIL import ImageGrab

root = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("binary", nargs="?", default=str(root / "target/release/zcall-gui"))
parser.add_argument("--output", default=str(root / "_validation/managed-update"))
args = parser.parse_args()
binary = pathlib.Path(args.binary).resolve()
display = os.environ.get("DISPLAY", "")
if not display or display == ":0":
    raise SystemExit("Use xvfb-run -a; host-display input is not allowed")
output = pathlib.Path(args.output).resolve()
output.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix="zcall-managed-ui-") as temp:
    temp = pathlib.Path(temp)
    tools = temp / "bin"
    tools.mkdir()
    home = temp / "home"
    home.mkdir()
    def program(name, body):
        path = tools / name
        path.write_text("#!" + sys.executable + "\n" + body)
        path.chmod(0o755)
        return path
    program("pacman", "import sys\nprint('zcalld-bin')\n")
    program("paru", "raise AssertionError('GUI must delegate gateway updates to zcalld')\n")
    program("xdg-terminal-exec", """import os,pathlib,sys,json
p=pathlib.Path(__file__).parent.parent
with (p/'terminal.jsonl').open('a') as f:f.write(json.dumps(sys.argv[1:])+'\\n')
a=sys.argv[1:]
if a[0]=='--':a=a[1:]
os.execv(a[0],a)
""")
    cli = program("zcalld", """import pathlib,sys,json,time
p=pathlib.Path(__file__).parent.parent
a=sys.argv[1:]
if a==['--version']:
 print('zcalld 0.2.0');raise SystemExit(0)
if a==['update','apply']:
 with (p/'helper.jsonl').open('a') as f:f.write(json.dumps(a)+'\\n')
 deadline=time.monotonic()+15
 while not (p/'allow-exit').exists():
  if time.monotonic()>=deadline:raise SystemExit(98)
  time.sleep(.02)
 raise SystemExit(int((p/'exit-code').read_text()))
with (p/'calls.jsonl').open('a') as f:f.write(json.dumps(a)+'\\n')
if a[:1]==['status']:r={'version':'0.2.0','healthy':False}
elif a[:2]==['update','check']:
 r={'managed':True,'method':'package-manager','latest':'0.2.1','available':True,'command':'touch '+str(p/'must-not-execute')}
elif a[:2]==['privacy','show']:r={'mode':'off'}
else:r={}
print(json.dumps(r))
""")
    (temp / "exit-code").write_text("17")
    env = dict(os.environ, PATH=str(tools)+os.pathsep+os.environ["PATH"],
               HOME=str(home), XDG_CONFIG_HOME=str(home/".config"),
               XDG_DATA_HOME=str(home/".local/share"), XDG_STATE_HOME=str(home/".local/state"),
               TERMINAL=str(tools/"xdg-terminal-exec"))
    proc = subprocess.Popen(
        [str(binary),"--zcalld",str(cli),"--page","updates","--language","en",
         "--theme","light","--smoke-seconds","45"],
        env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE,text=True)
    def xd(*args):
        return subprocess.check_output(["xdotool",*map(str,args)],text=True).strip()
    def lines(name):
        f=temp/name
        return [json.loads(x) for x in f.read_text().splitlines()] if f.exists() else []
    def until(predicate, message, timeout=10):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            if proc.poll() is not None:raise AssertionError(proc.stderr.read())
            if predicate():return
            time.sleep(.08)
        raise AssertionError(message)
    def pixels():return ImageGrab.grab(bbox=(0,0,1080,740),xdisplay=display)
    def notice_color():
        return pixels().getpixel((201,718))[:3]
    def red_notice():
        return notice_color() == (165,46,39)
    def busy_notice():
        return notice_color() == (153,79,44)
    try:
        window=""
        def locate():
            global window
            r=subprocess.run(["xdotool","search","--onlyvisible","--name","^zcall-gui"],
                             capture_output=True,text=True)
            if r.returncode==0:window=r.stdout.splitlines()[-1];return True
            return False
        until(locate,"Native window missing")
        xd("windowmove","--sync",window,0,0)
        xd("windowfocus","--sync",window)
        until(lambda:len(lines("calls.jsonl"))>=6,"Initial refresh missing")
        time.sleep(.3)
        def click():xd("mousemove","--window",window,960,192,"click",1);time.sleep(.25)
        xd("mousemove","--window",window,300,283,"click",1)
        until(lambda:any(a[:2]==["update","check"] for a in lines("calls.jsonl")),"Update check missing")
        time.sleep(.3)
        click()
        until(lambda:len(lines("helper.jsonl"))==1,"Package helper did not start")
        until(busy_notice,"Running helper was not shown in copper")
        assert not red_notice(),"Busy indicator was classified as an error"
        busy_color=notice_color()
        pixels().save(output/"busy.png")
        (temp/"allow-exit").touch()
        until(red_notice,"Failed helper was not shown as an error")
        failed_color=notice_color()
        assert not busy_notice(),"Failed indicator was classified as busy"
        failed_calls=len(lines("calls.jsonl"))
        pixels().save(output/"failure.png")
        (temp/"exit-code").write_text("0")
        click()
        until(lambda:len(lines("helper.jsonl"))==2,"Second package helper did not start")
        until(lambda:len(lines("calls.jsonl"))>=failed_calls+6,"Successful package update did not refresh state")
        until(lambda:not red_notice(),"Successful package result stayed in error state")
        time.sleep(.3)
        pixels().save(output/"success.png")
        helpers=lines("helper.jsonl")
        assert helpers==[["update","apply"],["update","apply"]],helpers
        assert not (temp/"must-not-execute").exists(),"JSON command was executed"
        for args in lines("terminal.jsonl"):
            assert args[6:] == [str(cli),"update","apply"],args
            result=pathlib.Path(args[5])
            assert result.name=="result" and not result.parent.exists(),args
        report={"scope":"private native Xvfb window; inert zcalld and terminal fixtures",
                "actual_system_upgrade":False,"helper_arguments":helpers,
                "busy_distinct_from_error":True,"busy_indicator_rgb":busy_color,
                "failure_indicator_rgb":failed_color,
                "nonzero_exit_displayed":True,"success_refreshed_state":True,
                "json_command_not_executed":True,"temporary_result_paths_removed":True}
        (output/"result.json").write_text(json.dumps(report,indent=2)+"\n")
        print(json.dumps(report,indent=2))
    finally:
        if proc.poll() is None:proc.terminate()
        try:proc.wait(timeout=5)
        except subprocess.TimeoutExpired:proc.kill();proc.wait()
