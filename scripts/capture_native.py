#!/usr/bin/env python3
"""Capture only a new owned native window; never move the host pointer."""
import argparse,json,os,pathlib,subprocess,tempfile,time
from PIL import ImageGrab
p=argparse.ArgumentParser();p.add_argument("binary");p.add_argument("--page",default="overview");p.add_argument("--output",required=True);p.add_argument("--zcalld");p.add_argument("--config");p.add_argument("--theme",default="light");p.add_argument("--language",default="zh-CN");a=p.parse_args()
def windows():
    r=subprocess.run(["xdotool","search","--name","^zcall-gui"],capture_output=True,text=True)
    return set(r.stdout.split()) if r.returncode==0 else set()
before=windows()
with tempfile.TemporaryDirectory(prefix="zcall-gui-capture-") as temp:
    fixture=pathlib.Path(temp)/"zcalld"
    fixture.write_text("""#!/usr/bin/env python3
import json,sys
g=sys.argv[1]
s={'status':{'version':'0.2.0','listen':'127.0.0.1:9415','healthy':False},'providers':{'providers':[{'name':'本地模型','base_url':'http://127.0.0.1:11434/v1','protocol':'chat','configured':True},{'name':'备用接口','base_url':'https://api.example.com/v1','protocol':'responses','configured':True}]},'agents':{'agents':[]},'plugins':{'plugins':[]},'privacy':{'mode':'secrets','enabled':True,'model_auto':False},'models':{'models':[]}}
print(json.dumps(s.get(g,{'ok':True})))
""");fixture.chmod(0o755)
    cmd=[str(pathlib.Path(a.binary).resolve()),"--page",a.page,"--zcalld",a.zcalld or str(fixture),"--smoke-seconds","7","--theme",a.theme,"--language",a.language]
    if a.config:cmd.extend(["--config",a.config])
    proc=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        owned=set()
        for _ in range(100):
            owned=windows()-before
            if owned:break
            time.sleep(.05)
        assert len(owned)==1,f"Expected one new GUI window, found {len(owned)}"
        wid=next(iter(owned));time.sleep(2)
        if os.environ.get("DISPLAY")!=":0":subprocess.run(["xdotool","windowmove","--sync",wid,"0","0"],check=True)
        raw=subprocess.check_output(["xdotool","getwindowgeometry","--shell",wid],text=True)
        geo=dict(line.split("=",1) for line in raw.splitlines() if "=" in line)
        x,y,w,h=[int(geo[k]) for k in ["X","Y","WIDTH","HEIGHT"]]
        output=pathlib.Path(a.output);output.parent.mkdir(parents=True,exist_ok=True)
        if os.environ.get("WAYLAND_DISPLAY") and os.environ.get("DISPLAY")==":0":
            clients=json.loads(subprocess.check_output(["hyprctl","clients","-j"],text=True))
            client=next(c for c in clients if c.get("pid")==proc.pid)
            x,y=map(int,client["at"]);w,h=map(int,client["size"])
            subprocess.run(["grim","-g",f"{x},{y} {w}x{h}",str(output)],check=True)
        else:
            ImageGrab.grab(bbox=(x,y,x+w,y+h),xdisplay=os.environ["DISPLAY"]).save(output)
        stdout,stderr=proc.communicate(timeout=8)
        assert proc.returncode==0,stderr
        output.with_suffix(".json").write_text(json.dumps({"renderer":stderr.strip(),"geometry":[x,y,w,h],"page":a.page,"theme":a.theme,"language":a.language,"data":"actual CLI with explicit config" if a.zcalld else "local fixture only","host_input":False},indent=2)+"\n")
        print(json.dumps({"image":str(output),"geometry":[x,y,w,h],"renderer":stderr.strip()}))
    finally:
        if proc.poll()is None:proc.terminate();proc.wait(timeout=5)
