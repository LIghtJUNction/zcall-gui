#!/usr/bin/env python3
"""Measure the GUI process on the selected native display, without model calls."""
import argparse,hashlib,json,os,pathlib,subprocess,tempfile,time
p=argparse.ArgumentParser()
p.add_argument("binary",nargs="?",default="target/release/zcall-gui")
p.add_argument("--output",default="_validation/hardware-idle.json")
args=p.parse_args();binary=pathlib.Path(args.binary).resolve()
def stats(pid):
    fields=pathlib.Path(f"/proc/{pid}/stat").read_text().rsplit(")",1)[1].split()
    memory={}
    for line in pathlib.Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
        if line.startswith(("Rss:","Pss:","Private_Clean:","Private_Dirty:","Swap:")):
            k,v,*_=line.split();memory[k.rstrip(":")]=int(v)
    drm=[];seen=set()
    for f in pathlib.Path(f"/proc/{pid}/fdinfo").iterdir():
        try:lines=[s for s in f.read_text().splitlines() if s.startswith("drm-")]
        except (OSError,PermissionError):continue
        identity=next((s for s in lines if s.startswith("drm-client-id:")),None)
        if lines and identity not in seen:drm.extend(lines);seen.add(identity)
    return sum(map(int,fields[11:13])),memory,drm
with tempfile.TemporaryDirectory(prefix="zcall-gui-hardware-") as directory:
    fake=pathlib.Path(directory)/"zcalld"
    fake.write_text("""#!/usr/bin/env python3
import json,sys
g=sys.argv[1]
values={'status':{'version':'fixture','listen':'127.0.0.1:9415','healthy':True},'providers':{'providers':[{'name':'local','base_url':'http://127.0.0.1:11434/v1','protocol':'chat','configured':True}]},'agents':{'agents':[]},'plugins':{'plugins':[]},'privacy':{'mode':'secrets','enabled':True},'models':{'models':[]}}
print(json.dumps(values.get(g,{'ok':True})))
""");fake.chmod(0o755)
    proc=subprocess.Popen([str(binary),"--zcalld",str(fake),"--page","providers","--smoke-seconds","9","--language","zh-CN","--theme","light"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        time.sleep(2)
        assert proc.poll() is None,("Native window exited early",proc.returncode,proc.communicate())
        before,_,_=stats(proc.pid);start=time.monotonic();time.sleep(3);after,memory,drm=stats(proc.pid)
        elapsed=time.monotonic()-start
        stdout,stderr=proc.communicate(timeout=8)
        assert proc.returncode==0,stderr
        result={"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"binary_bytes":binary.stat().st_size,"display":os.environ.get("DISPLAY"),"session":os.environ.get("XDG_SESSION_TYPE"),"renderer_output":stderr.strip(),"page":"providers","fixture":"local JSON only; no provider network calls","idle_seconds":elapsed,"cpu_ticks":after-before,"clock_ticks_per_second":os.sysconf("SC_CLK_TCK"),"cpu_percent_one_core":(after-before)/os.sysconf("SC_CLK_TCK")/elapsed*100,"memory_kib":memory,"drm_fdinfo":drm,"limits":"GUI process only; shared-library accounting is included; DRM metrics are driver-specific and are not a complete dedicated-VRAM measurement."}
        output=pathlib.Path(args.output);output.parent.mkdir(parents=True,exist_ok=True);output.write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result,indent=2))
    finally:
        if proc.poll() is None:proc.terminate();proc.wait(timeout=5)
