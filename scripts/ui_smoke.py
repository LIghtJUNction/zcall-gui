#!/usr/bin/env python3
"""Exercise the real window on a private X server; never send host input."""
import argparse,json,os,pathlib,subprocess,tempfile,time,sys
from PIL import ImageGrab
parser=argparse.ArgumentParser()
parser.add_argument("binary",nargs="?",default="target/release/zcall-gui")
parser.add_argument("--output",default="_validation")
parser.add_argument("--real-cli",help="Use an actual zcalld in an isolated home")
args=parser.parse_args()
binary=pathlib.Path(args.binary).resolve()
out=pathlib.Path(args.output).resolve();out.mkdir(parents=True,exist_ok=True)
display=os.environ.get("DISPLAY","")
if not display or display==":0":raise SystemExit("Use xvfb-run -a; host-display input is not allowed")
with tempfile.TemporaryDirectory(prefix="zcall-gui-ui-") as temp:
    temp=pathlib.Path(temp);fixture=temp/"zcalld";log=temp/"calls.jsonl";home=temp/"home";home.mkdir()
    env=os.environ.copy();env.update(HOME=str(home),XDG_CONFIG_HOME=str(home/".config"),XDG_DATA_HOME=str(home/".local/share"),XDG_STATE_HOME=str(home/".local/state"))
    config=home/".config/zcalld/config.conf"
    if args.real_cli:
        actual=str(pathlib.Path(args.real_cli).resolve())
        tools=temp/"bin";tools.mkdir();claude=tools/"claude";claude.write_text("#!/bin/sh\nexit 0\n");claude.chmod(0o755);env["PATH"]=str(tools)+os.pathsep+env["PATH"]
        subprocess.run([actual,"init","--config",str(config)],env=env,check=True,capture_output=True)
        body="""import json,pathlib,subprocess,sys
root=pathlib.Path(__file__).parent
a=sys.argv[1:];data=sys.stdin.buffer.read() if '--api-key-stdin' in a else b''
with (root/'calls.jsonl').open('a') as f:f.write(json.dumps({'args':a,'stdin_length':len(data)})+'\\n')
r=subprocess.run([ACTUAL,*a],input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
sys.stdout.buffer.write(r.stdout);sys.stderr.buffer.write(r.stderr);raise SystemExit(r.returncode)
""".replace("ACTUAL",repr(actual))
    else:
        body="""import json,sys,pathlib
root=pathlib.Path(__file__).parent
a=sys.argv[1:];key=sys.stdin.read() if '--api-key-stdin' in a else ''
with (root/'calls.jsonl').open('a') as f:f.write(json.dumps({'args':a,'stdin_length':len(key)})+'\\n')
s=json.loads((root/'state.json').read_text()) if (root/'state.json').exists() else {'providers':[{'name':'local','base_url':'http://127.0.0.1:11434/v1','protocol':'chat','configured':True}],'mode':'secrets','plugin_enabled':False,'agent_configured':False}
def option(k,default=''):return a[a.index(k)+1] if k in a else default
if a[:1]==['status']:r={'version':'0.2.0','listen':'127.0.0.1:9415','healthy':False}
elif a[:2]==['providers','list']:r={'providers':s['providers']}
elif a[:2]==['agents','list'] or a[:2]==['agents','detect']:r={'agents':[{'name':'claude','installed':True,'configured':s['agent_configured'],'route':'local','path':'~/.claude/settings.json'},{'name':'codex','installed':True,'configured':False,'path':'~/.codex/config.toml'}]}
elif a[:2]==['plugins','list']:r={'plugins':[{'name':'privacy','path':'/usr/lib/zcalld/plugins/privacy.so','kind':'native','enabled':s['plugin_enabled'],'version':'0.1.0'}]}
elif a[:2]==['privacy','show']:r={'mode':s['mode'],'enabled':s['mode']!='off','model_auto':False}
elif a[:1]==['models']:r={'models':[{'name':'local'}]}
elif a[:2]==['update','check']:r={'latest':'0.2.0','available':False}
else:
    if a[:2]==['providers','add']:s['providers'].append({'name':a[2],'base_url':option('--base-url'),'protocol':option('--protocol'),'configured':True})
    if a[:2]==['agents','apply']:s['agent_configured']=True
    if a[:2]==['agents','revert']:s['agent_configured']=False
    if a[:2]==['plugins','enable']:s['plugin_enabled']=True
    if a[:2]==['privacy','set']:s['mode']=a[2]
    (root/'state.json').write_text(json.dumps(s));r={'ok':True}
print(json.dumps(r))
"""
    fixture.write_text("#!"+sys.executable+"\n"+body);fixture.chmod(0o755)
    cmd=[str(binary),"--zcalld",str(fixture),"--config",str(config),"--smoke-seconds","60"]
    proc=subprocess.Popen(cmd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    def xd(*command):return subprocess.check_output(["xdotool",*map(str,command)],text=True).strip()
    def calls():return [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
    def count(prefix):return sum(c["args"][:len(prefix)]==prefix for c in calls())
    def wait_for(predicate,message,timeout=12):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            if proc.poll()is not None:raise AssertionError(f"GUI exited: {proc.returncode}; {proc.stderr.read()}")
            if predicate():return
            time.sleep(.06)
        raise AssertionError(message)
    def find_window():
        for _ in range(100):
            r=subprocess.run(["xdotool","search","--onlyvisible","--name","^zcall-gui"],capture_output=True,text=True)
            if r.returncode==0:return r.stdout.splitlines()[-1]
            time.sleep(.05)
        raise AssertionError("Native window missing")
    try:
        window=find_window();xd("windowmove","--sync",window,0,0);xd("windowfocus","--sync",window)
        wait_for(lambda:count(["models"])>=1,"Startup refresh did not finish");time.sleep(.3)
        def click(x,y):xd("mousemove","--window",window,x,y,"click",1);time.sleep(.12)
        def fill(x,y,text):click(x,y);xd("key","--window",window,"ctrl+a");xd("type","--window",window,"--clearmodifiers","--delay","2",text);time.sleep(.08)
        def shot(name):ImageGrab.grab(bbox=(0,0,1080,740),xdisplay=display).save(out/(name+".png"))
        def ready(previous):wait_for(lambda:count(["models"])>previous,"Action refresh did not finish");time.sleep(.25)
        def confirm():click(682,438)
        shot("overview")
        click(90,190);click(977,56)
        fill(820,248,"fixture-provider");fill(820,348,"http://127.0.0.1:9999/v1")
        shot("provider-step1");click(977,675)
        fill(820,248,"fixture-secret");fill(820,394,"fixture-model")
        shot("provider-step2");before=count(["models"]);click(977,675)
        wait_for(lambda:count(["providers","add","fixture-provider"])==1,"Provider form did not invoke CLI")
        add=next(c for c in calls() if c["args"][:3]==["providers","add","fixture-provider"])
        assert add["stdin_length"]==len("fixture-secret") and "fixture-secret" not in add["args"]
        ready(before);shot("providers")
        if args.real_cli:
            data=json.loads(subprocess.check_output([actual,"providers","list","--json","--config",str(config)],env=env))
            assert any(p["name"]=="fixture-provider" for p in data["providers"])
        click(90,238);fill(410,138,"local");shot("agents")
        before=count(["models"]);click(976,227);confirm()
        wait_for(lambda:count(["agents","apply","claude"])==1,"Agent apply did not invoke CLI");ready(before)
        before=count(["models"]);click(872,227);confirm()
        wait_for(lambda:count(["agents","revert","claude"])==1,"Agent restore did not invoke CLI");ready(before)
        click(90,286);shot("plugins")
        if not args.real_cli:
            click(450,180);before=count(["models"]);click(710,300);confirm()
            wait_for(lambda:count(["plugins","enable","privacy"])==1,"Plugin enable did not invoke CLI");ready(before)
        click(90,334);before=count(["models"]);click(600,417)
        wait_for(lambda:count(["privacy","set","personal"])==1,"Privacy action did not invoke CLI");ready(before);shot("privacy")
        if args.real_cli:
            data=json.loads(subprocess.check_output([actual,"privacy","show","--json","--config",str(config)],env=env))
            assert data["mode"]=="personal"
        click(90,382);shot("updates")
        if not args.real_cli:
            click(960,210);wait_for(lambda:count(["update","check"])==1,"Update check did not invoke CLI");time.sleep(.2)
        click(90,334);click(80,650);click(984,255);click(846,563)
        settings=home/".config/zcall-gui/settings.conf"
        wait_for(lambda:settings.exists() and "theme=dark" in settings.read_text() and "language=ja" in settings.read_text(),"Appearance not persisted")
        shot("settings-dark-ja");click(1035,45);shot("privacy-dark-ja")
        time.sleep(1)
        fields=pathlib.Path(f"/proc/{proc.pid}/stat").read_text().rsplit(")",1)[1].split()
        ticks=sum(map(int,fields[11:13]));time.sleep(2)
        fields=pathlib.Path(f"/proc/{proc.pid}/stat").read_text().rsplit(")",1)[1].split()
        idle=sum(map(int,fields[11:13]))-ticks
        first_stderr=proc.communicate(timeout=5) if proc.poll()is not None else None
        if proc.poll()is None:proc.terminate();first_stderr=proc.communicate(timeout=5)
        proc=subprocess.Popen(cmd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        window=find_window();xd("windowmove","--sync",window,0,0);time.sleep(.8)
        pixel=ImageGrab.grab(bbox=(200,90,201,91),xdisplay=display).getpixel((0,0))
        assert max(pixel[:3])<64,("Saved dark theme was not restored",pixel)
        report={"backend":"actual zcalld" if args.real_cli else "isolated fixture","display":"private Xvfb","provider_stdin_verified":True,"actual_provider_state_verified":bool(args.real_cli),"agent_apply_and_restore":True,"privacy_mode_verified":True,"theme_language_persisted":True,"restart_restores_dark":True,"idle_cpu_ticks_2s":idle,"clock_ticks_per_second":os.sysconf("SC_CLK_TCK"),"calls":len(calls()),"host_input":False}
        (out/"ui-smoke.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2),flush=True)
    finally:
        if proc.poll()is None:proc.terminate()
        try:stdout,stderr=proc.communicate(timeout=5)
        except subprocess.TimeoutExpired:proc.kill();stdout,stderr=proc.communicate()
        (out/"gui-stderr.log").write_text(stderr)
