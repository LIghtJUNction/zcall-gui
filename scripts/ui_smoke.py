#!/usr/bin/env python3
"""Exercise the real window on a private X server; never send host input."""
import argparse,ctypes,ctypes.util,hashlib,json,os,pathlib,subprocess,tempfile,time,sys
from PIL import ImageGrab

def click_without_motion(display, window, x, y):
    """Send only this fixture window's press/release; no later event may repaint it."""
    class Button(ctypes.Structure):
        _fields_ = [
            ("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int),
            ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("root", ctypes.c_ulong),
            ("subwindow", ctypes.c_ulong), ("time", ctypes.c_ulong),
            ("x", ctypes.c_int), ("y", ctypes.c_int),
            ("x_root", ctypes.c_int), ("y_root", ctypes.c_int),
            ("state", ctypes.c_uint), ("button", ctypes.c_uint), ("same_screen", ctypes.c_int),
        ]
    class Event(ctypes.Union):
        _fields_ = [("button", Button), ("pad", ctypes.c_long * 24)]
    library = ctypes.util.find_library("X11")
    if not library:
        raise AssertionError("X11 is required for the navigation repaint regression")
    x11 = ctypes.CDLL(library)
    for name, result, arguments in [
        ("XOpenDisplay", ctypes.c_void_p, [ctypes.c_char_p]),
        ("XCloseDisplay", ctypes.c_int, [ctypes.c_void_p]),
        ("XDefaultRootWindow", ctypes.c_ulong, [ctypes.c_void_p]),
        ("XSendEvent", ctypes.c_int, [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.POINTER(Event)]),
        ("XSync", ctypes.c_int, [ctypes.c_void_p, ctypes.c_int]),
    ]:
        function = getattr(x11, name);function.restype = result;function.argtypes = arguments
    connection = x11.XOpenDisplay(display.encode())
    if not connection:
        raise AssertionError("Private Xvfb display could not be opened")
    try:
        root = x11.XDefaultRootWindow(connection)
        for kind, mask in [(4, 1 << 2), (5, 1 << 3)]:
            event = Event()
            event.button.type = kind;event.button.display = connection
            event.button.window = int(window);event.button.root = root
            event.button.x = x;event.button.y = y
            event.button.x_root = x;event.button.y_root = y
            event.button.button = 1;event.button.same_screen = 1
            assert x11.XSendEvent(connection, int(window), 0, mask, ctypes.byref(event))
            x11.XSync(connection, 0)
    finally:
        x11.XCloseDisplay(connection)

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
    env={key:value for key,value in os.environ.items() if key in ("PATH","DISPLAY","XAUTHORITY","TMPDIR","TMP","TEMP","LANG","LC_ALL","LC_CTYPE","LIBGL_ALWAYS_SOFTWARE","GALLIUM_DRIVER")}
    runtime=home/"runtime";runtime.mkdir(mode=0o700)
    env.update(HOME=str(home),XDG_CONFIG_HOME=str(home/".config"),XDG_DATA_HOME=str(home/".local/share"),XDG_STATE_HOME=str(home/".local/state"),XDG_RUNTIME_DIR=str(runtime))
    config=home/".config/zcalld/config.conf"
    if args.real_cli:
        actual=str(pathlib.Path(args.real_cli).resolve())
        tools=temp/"bin";tools.mkdir();claude=tools/"claude";claude.write_text("#!/bin/sh\nexit 0\n");claude.chmod(0o755);env["PATH"]=str(tools)+os.pathsep+env["PATH"]
        subprocess.run([actual,"init","--config",str(config)],env=env,check=True,capture_output=True)
        initial=config.read_text();assert "[route local]" in initial
        initial=initial.replace("listen = 127.0.0.1:9415", "listen = 127.0.0.1:0")
        initial=initial.replace("unix_socket = auto", "unix_socket = none")
        initial=initial.replace("[route local]","[route starter-route]")
        initial+="\n[route writing-special]\nprovider = local\nmodel = fixture-model\n"
        for index in range(8):
            initial+=f"\n[route extra-{index:02d}]\nprovider = local\nmodel = fixture-{index}\n"
        config.write_text(initial)
        original_agent=home/".claude/settings.json";original_agent.parent.mkdir(parents=True)
        original_bytes=b'{"theme":"preserve-user-value"}\n'
        original_agent.write_bytes(original_bytes);original_agent.chmod(0o600)
        subprocess.run([actual,"agents","apply","claude","--route","starter-route","--json","--config",str(config)],env=env,check=True,capture_output=True)
        # Revert restores the configuration immediately before this GUI change.
        before_gui_bytes=original_agent.read_bytes()
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
s=json.loads((root/'state.json').read_text()) if (root/'state.json').exists() else {'providers':[{'name':'local','base_url':'http://127.0.0.1:11434/v1','protocol':'chat','configured':True}],'mode':'secrets','plugin_enabled':False,'agent_configured':True,'agent_route':'starter-route','routes':[{'id':'starter-route','name':'Starter model'},{'id':'writing-special','name':'Writing model'}]+[{'id':'extra-%02d'%i,'name':'Extra %02d'%i} for i in range(8)]}
def option(k,default=''):return a[a.index(k)+1] if k in a else default
if a[:1]==['status']:r={'version':'0.2.0','listen':'127.0.0.1:9415','healthy':False}
elif a[:2]==['providers','list']:r={'providers':s['providers']}
elif a[:2]==['agents','list'] or a[:2]==['agents','detect']:r={'agents':[{'name':'claude','installed':True,'configured':s['agent_configured'],'writable':True,'route':s.get('agent_route'),'path':'~/.claude/settings.json'},{'name':'codex','installed':True,'configured':False,'writable':True,'path':'~/.codex/config.toml'},{'name':'cursor','installed':True,'configured':False,'writable':False}]}
elif a[:2]==['plugins','list']:r={'plugins':[{'name':'privacy','path':'/usr/lib/zcalld/plugins/privacy.so','kind':'native','enabled':s['plugin_enabled'],'version':'0.1.0'}]}
elif a[:2]==['privacy','show']:r={'mode':s['mode'],'enabled':s['mode']!='off','model_auto':False}
elif a[:1]==['models']:r={'object':'list','data':s['routes']}
elif a[:2]==['update','check']:r={'latest':'0.2.0','available':False}
else:
    if a[:2]==['providers','add']:s['providers'].append({'name':a[2],'base_url':option('--base-url'),'protocol':option('--protocol'),'configured':True})
    if a[:2]==['agents','apply']:
        route=option('--route')
        if route not in {m['id'] for m in s['routes']}:print(json.dumps({'error':'route not available'}));raise SystemExit(1)
        s['agent_configured']=True;s['agent_route']=route
    if a[:2]==['agents','revert']:s['agent_configured']=False;s['agent_route']=None
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
        navigation=[]
        def navigate(x,y,page,label):
            click_without_motion(display,window,x,y)
            def selected():
                frame=ImageGrab.grab(bbox=(0,0,1080,740),xdisplay=display)
                markers=[frame.getpixel((13,142+row*48)) for row in range(6)]
                others=[color for row,color in enumerate(markers) if row!=page]
                return len(set(others))==1 and markers[page]!=others[0]
            try:
                # Capture/query only: pointer, keyboard and Expose events would mask the bug.
                wait_for(selected,"Navigation did not repaint the selected page: "+label,timeout=2)
            except AssertionError:
                shot("navigation-failed-"+label)
                raise
            frame=ImageGrab.grab(bbox=(0,0,1080,740),xdisplay=display)
            navigation.append({"case":label,"selected_page":page,"header_sha256":hashlib.sha256(frame.crop((198,26,520,70)).tobytes()).hexdigest()})
        reads_before_navigation=len(calls())
        for page in [1,2,3,4,5,0]:
            navigate(80,142+page*48,page,"sidebar-"+str(page))
        assert len({item["header_sha256"] for item in navigation})==6,"Page content did not change with navigation"
        assert len(calls())==reads_before_navigation,"Navigation unexpectedly invoked the CLI"
        shot("overview")
        navigate(410,400,1,"overview-providers");click(977,56)
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
        navigate(80,142,0,"return-overview");navigate(840,400,2,"overview-agents");shot("agents")
        before=count(["models"]);click(976,227);shot("agent-route-picker")
        # The second Agent's background button must not replace the picker owner.
        click(976,331)
        xd("key","--window",window,"Tab")
        xd("type","--window",window,"--clearmodifiers","--delay","2","no-such-route")
        xd("key","--window",window,"Return");time.sleep(.12)
        assert count(["agents","apply"])==0,"Searching must not apply a route"
        shot("agent-route-no-match")
        fill(500,185,"writing")
        xd("key","--window",window,"Tab");xd("key","--window",window,"Return");time.sleep(.12)
        shot("agent-route-selected")
        xd("key","--window",window,"Tab");xd("key","--window",window,"Return");time.sleep(.12)
        assert count(["agents","apply"])==0,"Selecting a route must still require confirmation"
        shot("agent-route-confirm");confirm()
        wait_for(lambda:count(["agents","apply","claude"])==1,"Search-selected Agent apply did not invoke CLI");ready(before)
        apply=next(c for c in calls() if c["args"][:3]==["agents","apply","claude"])
        assert apply["args"][apply["args"].index("--route")+1]=="writing-special",apply
        assert count(["agents","apply","codex"])==0,"Picker allowed a background Agent click"
        shot("agents-current-route")
        before=count(["models"]);click(872,227)
        assert count(["agents","revert","claude"])==0,"Restore must require confirmation"
        confirm()
        wait_for(lambda:count(["agents","revert","claude"])==1,"Agent restore did not invoke CLI");ready(before)
        catalog_unavailable=False
        if args.real_cli:
            assert original_agent.read_bytes()==before_gui_bytes,"Restore lost the pre-change user configuration"
            assert json.loads(original_agent.read_text())["theme"]=="preserve-user-value"
            assert original_agent.stat().st_mode&0o777==0o600
            saved=config.read_text();lines=[];in_route=False
            for line in saved.splitlines(keepends=True):
                if line.strip().startswith("["):in_route=line.strip().startswith("[route ")
                if not in_route:lines.append(line)
            config.write_text("".join(lines))
            probe=subprocess.run([actual,"models","--json","--config",str(config)],env=env,capture_output=True,text=True)
            catalog_unavailable=probe.returncode!=0
            if catalog_unavailable:
                assert json.loads(probe.stdout).get("error"),"A failed catalog read must report its error"
            else:
                assert json.loads(probe.stdout).get("data")==[],"Expected an empty model catalog"
        else:
            state_file=temp/"state.json";saved=state_file.read_text()
            state=json.loads(saved);state["routes"]=[];state_file.write_text(json.dumps(state))
        before=count(["models"]);click(1012,713);ready(before)
        previous=count(["agents","apply"]);click(976,227);time.sleep(.12)
        assert count(["agents","apply"])==previous,"An empty catalog must disable apply"
        shot("agents-no-routes")
        if catalog_unavailable:
            # Strict core versions reject a configuration with no routes. This
            # is a failed read, whose CTA must retry rather than start setup.
            config.write_text(saved)
            before=count(["models"]);click(968,143);ready(before)
            shot("agents-catalog-recovered")
        else:
            navigate(968,143,1,"empty-catalog-add-provider");shot("agents-add-provider-help");click(1035,45)
            if args.real_cli:config.write_text(saved)
            else:state_file.write_text(saved)
            before=count(["models"]);click(1012,713);ready(before)
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
            click(300,283);wait_for(lambda:count(["update","check"])==1,"Update check did not invoke CLI");time.sleep(.2)
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
        report={"backend":"actual zcalld" if args.real_cli else "isolated fixture","display":"private Xvfb","overview_provider_and_agent_navigation":True,"navigation_without_followup_input":navigation,"provider_stdin_verified":True,"actual_provider_state_verified":bool(args.real_cli),"agent_apply_and_restore":True,"catalog_uses_real_data_id":True,"no_guessed_local_route":True,"route_search_keyboard":True,"route_picker_blocks_background":True,"empty_catalog_disables_apply":not catalog_unavailable,"unavailable_catalog_disables_apply":catalog_unavailable,"failed_catalog_retry":catalog_unavailable,"empty_catalog_add_provider":not catalog_unavailable,"agent_pre_change_config_preserved":bool(args.real_cli),"privacy_mode_verified":True,"theme_language_persisted":True,"restart_restores_dark":True,"idle_cpu_ticks_2s":idle,"clock_ticks_per_second":os.sysconf("SC_CLK_TCK"),"calls":len(calls()),"host_input":False}
        (out/"ui-smoke.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2),flush=True)
    finally:
        if proc.poll()is None:proc.terminate()
        try:stdout,stderr=proc.communicate(timeout=5)
        except subprocess.TimeoutExpired:proc.kill();stdout,stderr=proc.communicate()
        (out/"gui-stderr.log").write_text(stderr)
