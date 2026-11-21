#!/usr/bin/env python3
"""Exercise loading, failure and retry UI with an isolated, synthetic CLI."""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

from PIL import ImageGrab


FIXTURE = r'''import json,pathlib,sys,time
root=pathlib.Path(__file__).parent
args=sys.argv[1:]
mode=(root/'scenario').read_text().strip()
with (root/'calls.jsonl').open('a') as log:
    log.write(json.dumps(args)+'\n')
if (mode=='loading' and args[:1]==['status']) or (mode=='usage-loading' and args[:2]==['usage','summary']):
    (root/'loading-started').touch()
    deadline=time.monotonic()+10
    while not (root/'release-loading').exists() and time.monotonic()<deadline:
        time.sleep(.05)
    if not (root/'release-loading').exists():
        print(json.dumps({'error':'loading fixture was not released'}));raise SystemExit(1)
status={'version':'0.3.1','healthy':mode!='offline','listen':'127.0.0.1:3425'}
if mode=='endpoint-unknown':status.pop('listen')
responses={
    'status':status,
    'providers':{'providers':[] if mode=='empty-providers' else [{'name':'Fixture endpoint','base_url':'http://127.0.0.1:11434/v1','protocol':'chat','configured':True}]},
    'agents':{'agents':[{'name':'codex','installed':True,'configured':False,'writable':True}]},
    'plugins':{'plugins':[]},
    'privacy':{} if mode=='privacy-unavailable' else {'mode':'secrets','enabled':True},
    'models':{'object':'list','data':[] if mode=='empty-catalog' else [{'id':'fixture-model','name':'Fixture model'}]},
    'usage':{'requests':0,'failed':0,'input_tokens':0,'output_tokens':0} if mode in ('empty-usage','recording-disabled','legacy-usage') else ({'requests':'invalid'} if mode=='invalid-usage' else {'requests':1250,'failed':5,'input_tokens':480000,'output_tokens':96000,'cache_read_tokens':160000,'cache_write_tokens':24000,'reasoning_tokens':18000}),
}
if mode!='legacy-usage':responses['usage']['recording_configured']=mode!='recording-disabled'
failures={'unavailable':'status','providers-unavailable':'providers','catalog-unavailable':'models','usage-unavailable':'usage'}
if failures.get(mode)==args[0]:
    print(json.dumps({'error':'Synthetic local read failed; retry is available.'}));raise SystemExit(1)
if args[0] not in responses:
    print(json.dumps({'error':'Unexpected write in read-only state fixture'}));raise SystemExit(1)
print(json.dumps(responses[args[0]]))
'''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", nargs="?", default="target/release/zcall-gui")
    parser.add_argument("--output", default="_validation/state-ui")
    parser.add_argument("--theme", choices=["light", "dark"], default="dark")
    parser.add_argument("--language", default="zh-CN")
    parser.add_argument("--only-usage", action="store_true")
    args = parser.parse_args()
    display = os.environ.get("DISPLAY", "")
    if not display or display in (":0", ":0.0"):
        raise SystemExit("Use xvfb-run -a; host-display input is not allowed")
    binary = pathlib.Path(args.binary).resolve()
    output = pathlib.Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    results = []
    cases = [
        ("loading", "overview", None),
        ("loading", "providers", None),
        ("unavailable", "overview", (300, 290)),
        ("offline", "overview", None),
        ("healthy", "overview", None),
        ("endpoint-unknown", "overview", None),
        ("providers-unavailable", "providers", (285, 286)),
        ("empty-providers", "providers", None),
        ("catalog-unavailable", "agents", (965, 143)),
        ("empty-catalog", "agents", None),
        ("privacy-unavailable", "privacy", None),
        ("usage-loading", "usage", None),
        ("healthy", "usage", None),
        ("empty-usage", "usage", None),
        ("recording-disabled", "usage", None),
        ("legacy-usage", "usage", None),
        ("usage-unavailable", "usage", (285, 290)),
        ("invalid-usage", "usage", (285, 290)),
    ]

    if args.only_usage:
        cases = [case for case in cases if case[1] == "usage"]

    def xd(*command):
        return subprocess.check_output(
            ["xdotool", *map(str, command)], text=True
        ).strip()

    def windows():
        found = subprocess.run(
            ["xdotool", "search", "--onlyvisible", "--name", "^zcall-gui"],
            capture_output=True, text=True,
        )
        return set(found.stdout.split()) if found.returncode == 0 else set()

    for mode, page, retry_point in cases:
        with tempfile.TemporaryDirectory(prefix="zcall-gui-state-") as temp:
            root = pathlib.Path(temp)
            fixture = root / "zcalld"
            fixture.write_text("#!" + sys.executable + "\n" + FIXTURE)
            fixture.chmod(0o755)
            scenario = root / "scenario"
            scenario.write_text(mode)
            home = root / "home"
            home.mkdir()
            env = os.environ.copy()
            env.update(
                HOME=str(home),
                XDG_CONFIG_HOME=str(home / ".config"),
                XDG_DATA_HOME=str(home / ".local/share"),
                XDG_STATE_HOME=str(home / ".local/state"),
            )
            command = [
                str(binary), "--zcalld", str(fixture), "--page", page,
                "--theme", args.theme, "--language", args.language,
                "--smoke-seconds", "20",
            ]
            diagnostic = root / "renderer.log"
            before = windows()
            with diagnostic.open("w") as stream:
                proc = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=stream)

                def wait_for(predicate, message, timeout=12):
                    deadline = time.monotonic() + timeout
                    while time.monotonic() < deadline:
                        if proc.poll() is not None:
                            raise AssertionError(message + ": " + diagnostic.read_text())
                        if predicate():
                            return
                        time.sleep(.05)
                    raise AssertionError(message)

                def calls():
                    log = root / "calls.jsonl"
                    return [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []

                def capture(name, window):
                    geometry = dict(
                        line.split("=", 1) for line in xd("getwindowgeometry", "--shell", window).splitlines()
                        if "=" in line
                    )
                    x, y, width, height = [int(geometry[key]) for key in ("X", "Y", "WIDTH", "HEIGHT")]
                    ImageGrab.grab(bbox=(x, y, x + width, y + height), xdisplay=display).save(output / (name + ".png"))

                name = f"{page}-{mode}-{args.theme}-{args.language}"
                try:
                    wait_for(lambda: len(windows() - before) == 1, "Native state window missing")
                    window = next(iter(windows() - before))
                    xd("windowmove", "--sync", window, 0, 0)
                    if mode in ("loading", "usage-loading"):
                        wait_for(lambda: (root / "loading-started").exists(), "Delayed status did not begin")
                        time.sleep(.2)
                        capture(name, window)
                        (root / "release-loading").touch()
                    wait_for(
                        lambda: "Smoke ready: state loaded and presented" in diagnostic.read_text(),
                        "Completed snapshot was not presented",
                    )
                    if mode not in ("loading", "usage-loading"):
                        capture(name, window)
                    if retry_point:
                        previous = len(calls())
                        scenario.write_text("healthy")
                        xd("mousemove", "--window", window, *retry_point, "click", 1)
                        wait_for(lambda: len(calls()) >= previous + (1 if page == "usage" else 6), "Retry did not refresh the local snapshot")
                        time.sleep(.25)
                        capture(name + "-recovered", window)
                    allowed = {
                        ("status",), ("providers", "list"), ("agents", "list"),
                        ("plugins", "list"), ("privacy", "show"), ("models",), ("usage", "summary"),
                    }
                    assert all(tuple(call[:call.index("--json")]) in allowed for call in calls()), calls()
                    # Event handling must not repeatedly rescan local usage history.
                    previous_reads = len(calls())
                    time.sleep(.3)
                    assert len(calls()) == previous_reads, "Idle page invoked the CLI"
                    # Only the owned fixture window receives the close key.
                    subprocess.run(
                        ["xdotool", "key", "--window", window, "ctrl+q"],
                        check=False, stderr=subprocess.DEVNULL,
                    )
                    proc.communicate(timeout=5)
                    assert proc.returncode == 0, diagnostic.read_text()
                    results.append({
                        "case": name, "retry_refreshed": bool(retry_point),
                        "read_only_calls": len(calls()), "renderer": diagnostic.read_text().strip(),
                    })
                finally:
                    (root / "release-loading").touch()
                    if proc.poll() is None:
                        proc.terminate()
                        try:
                            proc.communicate(timeout=5)
                        except subprocess.TimeoutExpired:
                            proc.kill()
                            proc.communicate(timeout=5)
    report = {
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "display": "private Xvfb", "data": "synthetic local CLI only",
        "host_input": False, "cases": results,
    }
    (output / "state-ui.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
