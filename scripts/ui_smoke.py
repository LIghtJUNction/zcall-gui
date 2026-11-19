#!/usr/bin/env python3
"""Exercise a real X11 GUI against an isolated command-line fixture."""
import argparse,json,os,pathlib,subprocess,tempfile,time,sys
from PIL import ImageGrab
parser=argparse.ArgumentParser()
parser.add_argument("binary",nargs="?",default="target/release/zcall-gui")
parser.add_argument("--output",default="_validation")
args=parser.parse_args()
binary=pathlib.Path(args.binary).resolve()
out=pathlib.Path(args.output).resolve();out.mkdir(parents=True,exist_ok=True)
if not os.environ.get("DISPLAY"):raise SystemExit("Run with xvfb-run -a to use an isolated display")
with tempfile.TemporaryDirectory(prefix="zcall-gui-ui-") as temp:
    temp=pathlib.Path(temp);fixture=temp/"zcalld";log=temp/"calls.jsonl";state=temp/"state.json"
    fixture.write_text("""#!/usr/bin/env python3
import json,os,sys,pathlib
root=pathlib.Path(__file__).parent
a=sys.argv[1:]; key=sys.stdin.read() if '--api-key-stdin' in a else ''
with (root/'calls.jsonl').open('a') as f:f.write(json.dumps({'args':a,'stdin_length':len(key)})+'\\n')
s=json.loads((root/'state.json').read_text()) if (root/'state.json').exists() else {'providers':[{'name':'local','base_url':'http://127.0.0.1:11434/v1','protocol':'chat','configured':True}],'mode':'secrets','plugin_enabled':False}
def option(k,default=''):return a[a.index(k)+1] if k in a else default
if a[:1]==['status']:r={'version':'0.2.0','listen':'127.0.0.1:9415','healthy':True}
elif a[:2]==['providers','list']:r={'providers':s['providers']}
elif a[:2]==['agents','list'] or a[:2]==['agents','detect']:r={'agents':[{'name':'claude','installed':True,'configured':True,'route':'local','path':'~/.claude/settings.json'},{'name':'codex','installed':True,'configured':False,'path':'~/.codex/config.toml'}]}
elif a[:2]==['plugins','list']:r={'plugins':[{'name':'privacy','path':'/usr/lib/zcalld/plugins/privacy.so','kind':'native','enabled':s['plugin_enabled'],'version':'0.1.0'}]}
elif a[:2]==['privacy','show']:r={'mode':s['mode'],'enabled':s['mode']!='off','model_auto':False}
elif a[:1]==['models']:r={'models':[{'name':'local'}]}
elif a[:2]==['update','check']:r={'latest':'0.2.0','available':False}
else:
    if a[:2]==['providers','add']:s['providers'].append({'name':a[2],'base_url':option('--base-url'),'protocol':option('--protocol'),'configured':True})
    if a[:2]==['plugins','enable']:s['plugin_enabled']=True
    if a[:2]==['privacy','set']:s['mode']=a[2]
    (root/'state.json').write_text(json.dumps(s))
    r={'ok':True}
print(json.dumps(r))
""")
    fixture.chmod(0o755)
    proc=subprocess.Popen([str(binary),"--zcalld",str(fixture),"--smoke-seconds","35"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    def xd(*command):return subprocess.check_output(["xdotool",*map(str,command)],text=True).strip()
    def calls():
        return [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
    def wait_for(predicate,message,timeout=8):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            if proc.poll() is not None:raise AssertionError(f"GUI exited early: {proc.returncode}; {proc.stderr.read()}")
            if predicate():return
            time.sleep(.06)
        raise AssertionError(message)
    def saw(prefix):return any(c['args'][:len(prefix)]==prefix for c in calls())
    try:
        window=""
        for _ in range(100):
            result=subprocess.run(["xdotool","search","--name","^zcall-gui"],text=True,capture_output=True)
            if result.returncode==0:window=result.stdout.splitlines()[0];break
            time.sleep(.05)
        assert window,"Native GUI window was not created"
        xd("windowmove","--sync",window,0,0)
        xd("windowfocus","--sync",window)
        wait_for(lambda:saw(["privacy","show"]),"Startup state was not loaded")
        time.sleep(.2)
        def click(x,y):xd("mousemove","--window",window,x,y,"click",1);time.sleep(.12)
        def fill(x,y,text):click(x,y);xd("key","--window",window,"ctrl+a");xd("type","--window",window,"--clearmodifiers",text);time.sleep(.08)
        def shot(name):ImageGrab.grab(xdisplay=os.environ["DISPLAY"]).save(out/(name+".png"))
        shot("overview")
        click(90,194)
        fill(830,222,"fixture-provider")
        fill(830,291,"http://127.0.0.1:9999/v1")
        fill(830,435,"fixture-model")
        fill(830,507,"fixture-secret")
        shot("providers")
        click(860,568)
        wait_for(lambda:saw(["providers","add","fixture-provider"]),"Provider form did not invoke CLI")
        add=next(c for c in calls() if c["args"][:3]==["providers","add","fixture-provider"])
        assert add["stdin_length"]==len("fixture-secret") and "fixture-secret" not in add["args"]
        wait_for(lambda:len([c for c in calls() if c["args"][:2]==["privacy","show"]])>=2,"Provider refresh did not finish")
        time.sleep(.2)
        click(90,243);fill(450,169,"fixture-route");shot("agents");click(890,262);click(711,430)
        wait_for(lambda:saw(["agents","apply","claude"]),"Agent action did not invoke CLI")
        apply=next(c for c in calls() if c["args"][:3]==["agents","apply","claude"])
        assert "--route" in apply["args"] and "fixture-route" in apply["args"]
        time.sleep(.5)
        click(90,292);shot("plugins");click(628,183);click(711,430)
        wait_for(lambda:saw(["plugins","enable","privacy"]),"Plugin action did not invoke CLI")
        time.sleep(.5)
        click(90,341);shot("privacy");click(992,412)
        wait_for(lambda:saw(["privacy","set","personal"]),"Privacy mode did not invoke CLI")
        time.sleep(.5)
        click(90,390);shot("updates");click(311,261)
        wait_for(lambda:saw(["update","check"]),"Gateway update check did not invoke CLI")
        time.sleep(.3)
        before=pathlib.Path(f"/proc/{proc.pid}/stat").read_text().rsplit(")",1)[1].split()
        time.sleep(2)
        after=pathlib.Path(f"/proc/{proc.pid}/stat").read_text().rsplit(")",1)[1].split()
        cpu_ticks=sum(map(int,after[11:13]))-sum(map(int,before[11:13]))
        memory={}
        for line in pathlib.Path(f"/proc/{proc.pid}/smaps_rollup").read_text().splitlines():
            if line.startswith(("Rss:","Pss:","Private_Clean:","Private_Dirty:")):
                key,value,*_=line.split();memory[key.rstrip(":")]=int(value)
        report={"binary":str(binary),"display":"isolated Xvfb","screens":6,"actions":["provider credentials via stdin","agent route apply","plugin enable","privacy mode","gateway update check"],"calls":len(calls()),"idle_cpu_ticks_2s":cpu_ticks,"clock_ticks_per_second":os.sysconf("SC_CLK_TCK"),"memory_kib":memory,"secrets_in_argv":False}
        (out/"ui-smoke.json").write_text(json.dumps(report,indent=2)+"\n")
        print(json.dumps(report,indent=2),flush=True)
    finally:
        proc.terminate()
        try:stdout,stderr=proc.communicate(timeout=5)
        except subprocess.TimeoutExpired:proc.kill();stdout,stderr=proc.communicate()
        (out/"gui-stderr.log").write_text(stderr)
