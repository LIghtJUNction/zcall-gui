#!/usr/bin/env python3
"""Capture Agent identities and list navigation on an isolated Xvfb display."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from PIL import ImageGrab


AGENTS = [
    "claude", "codex", "gemini", "opencode", "pi", "qwen", "aider", "goose",
    "zed", "vscode", "cursor", "amp", "continue", "cline", "roo", "kilo",
    "自定义智能体・日本語エージェント・很长的未知工具名称・長い名前の確認用ツール" * 3,
]
READS = {
    ("status",), ("providers", "list"), ("agents", "list"),
    ("plugins", "list"), ("privacy", "show"), ("models",),
}
CASES = [
    (1080, 740, "dark", "zh-CN"),
    (1080, 740, "light", "zh-CN"),
    (1080, 740, "dark", "en"),
    (1080, 740, "light", "en"),
    (720, 560, "dark", "ja"),
    (640, 560, "light", "en"),
    (900, 640, "light", "zh-TW"),
]
FIXTURE = r'''import json,pathlib,sys
root=pathlib.Path(__file__).parent
args=sys.argv[1:]
with (root/'calls.jsonl').open('a',encoding='utf-8') as log:
    log.write(json.dumps(args,ensure_ascii=False)+'\n')
command=tuple(args[:args.index('--json')]) if '--json' in args else tuple(args)
allowed={('status',),('providers','list'),('agents','list'),('plugins','list'),('privacy','show'),('models',)}
if command not in allowed:
    print(json.dumps({'error':'Unexpected write in the Agent identity fixture'}));raise SystemExit(1)
print(json.dumps(json.loads((root/'snapshot.json').read_text(encoding='utf-8'))[command[0]],ensure_ascii=False))
'''


def snapshot():
    route = "team/非常长的模型路由/長いモデルルート/" + "planning-and-code-review/" * 7
    agents = []
    for index, name in enumerate(AGENTS):
        state = index % 4
        agents.append({
            "name": name,
            "installed": state != 3,
            "configured": state == 0,
            "writable": state in (0, 1),
            "route": route if state in (0, 1) else None,
            "path": f"~/.fixture-agent-{index}/settings.json",
        })
    return {
        "status": {"version": "0.4.0", "healthy": True, "listen": "127.0.0.1:3425"},
        "providers": {"providers": [{"name": "Fixture", "configured": True}]},
        "agents": {"agents": agents},
        "plugins": {"plugins": []},
        "privacy": {"mode": "secrets", "enabled": True},
        "models": {"object": "list", "data": [{"id": route, "name": "Fixture model"}]},
    }


def xd(*args):
    return subprocess.check_output(["xdotool", *map(str, args)], text=True).strip()


def windows():
    result = subprocess.run(
        ["xdotool", "search", "--onlyvisible", "--name", "^zcall-gui"],
        capture_output=True, text=True, check=False,
    )
    return set(result.stdout.split()) if result.returncode == 0 else set()


def geometry(window):
    fields = dict(
        line.split("=", 1)
        for line in xd("getwindowgeometry", "--shell", window).splitlines()
        if "=" in line
    )
    return tuple(int(fields[key]) for key in ("X", "Y", "WIDTH", "HEIGHT"))


def layout(width, height):
    available = width - 166 - 64
    content_width = min(1024, available)
    left = round(198 + (available - content_width) / 2)
    narrow = content_width < 620
    step, row_height = (152, 140) if narrow else (104, 92)
    # The final row must leave sixteen pixels above the paging controls.
    rows = max(1, (height - 82 - 16 - 189 - row_height) // step + 1)
    return left, content_width, step, rows


def image_hash(image):
    return hashlib.sha256(image.tobytes()).hexdigest()


def logo_pixels(frame, left, top):
    # Inspect the mark inside its tile, independently of theme and brand color.
    mark = frame.crop((left + 5, top + 5, left + 35, top + 35)).convert("RGB")
    colors = Counter(zip(*(iter(mark.tobytes()),) * 3))
    background, _ = colors.most_common(1)[0]
    distinct = sum(
        count for color, count in colors.items()
        if max(abs(a - b) for a, b in zip(color, background)) >= 24
    )
    assert distinct >= 8, f"Empty Agent logo at ({left}, {top}); contrasting pixels={distinct}"
    return {"contrasting_pixels": distinct, "sha256": image_hash(mark)}


def run_case(binary, output, display, case):
    width, height, theme, language = case
    name = f"agents-{width}x{height}-{theme}-{language}"
    with tempfile.TemporaryDirectory(prefix="zcall-gui-identities-") as temp:
        root = Path(temp)
        home = root / "home"
        home.mkdir()
        runtime = home / "runtime"
        runtime.mkdir(mode=0o700)
        fixture = root / "zcalld"
        fixture.write_text("#!" + sys.executable + "\n" + FIXTURE, encoding="utf-8")
        fixture.chmod(0o755)
        (root / "snapshot.json").write_text(json.dumps(snapshot(), ensure_ascii=False), encoding="utf-8")
        env = {
            key: value for key, value in os.environ.items()
            if key in {
                "PATH", "DISPLAY", "XAUTHORITY", "TMPDIR", "TMP", "TEMP",
                "LANG", "LC_ALL", "LC_CTYPE", "LIBGL_ALWAYS_SOFTWARE", "GALLIUM_DRIVER",
            }
        }
        env.update(
            HOME=str(home), XDG_CONFIG_HOME=str(home / ".config"),
            XDG_DATA_HOME=str(home / ".local/share"),
            XDG_STATE_HOME=str(home / ".local/state"), XDG_RUNTIME_DIR=str(runtime),
        )
        command = [
            str(binary), "--zcalld", str(fixture), "--page", "agents",
            "--theme", theme, "--language", language, "--smoke-seconds", "90",
        ]
        diagnostic = output / (name + ".log")
        before = windows()
        with diagnostic.open("w", encoding="utf-8") as log:
            proc = subprocess.Popen(command, env=env, stdout=subprocess.DEVNULL, stderr=log)

            def wait_for(predicate, message, timeout=12):
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    if proc.poll() is not None:
                        raise AssertionError(message + ": " + diagnostic.read_text(encoding="utf-8"))
                    if predicate():
                        return
                    time.sleep(.04)
                raise AssertionError(message)

            def calls():
                path = root / "calls.jsonl"
                return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()] if path.exists() else []

            try:
                wait_for(lambda: len(windows() - before) == 1, "Native Agent window missing")
                window = next(iter(windows() - before))
                xd("windowmove", "--sync", window, 0, 0)
                xd("windowsize", "--sync", window, width, height)
                wait_for(lambda: geometry(window)[2:] == (width, height), "Window did not accept the test size")
                wait_for(
                    lambda: "Smoke ready: state loaded and presented" in diagnostic.read_text(encoding="utf-8"),
                    "Agent snapshot was not presented",
                )
                time.sleep(.15)
                left, content_width, step, rows = layout(width, height)
                final_offset = max(0, len(AGENTS) - rows)
                pages, seen, signatures = [], set(), {}

                def frame():
                    x, y, actual_width, actual_height = geometry(window)
                    assert (actual_width, actual_height) == (width, height)
                    return ImageGrab.grab(bbox=(x, y, x + actual_width, y + actual_height), xdisplay=display)

                def row_signature(image, position=0):
                    y = 189 + position * step
                    return image_hash(image.crop((left + 14, y + 6, left + min(content_width - 16, 400), y + 78)))

                def capture(offset, suffix):
                    image = frame()
                    path = output / (name + "-" + suffix + ".png")
                    image.save(path)
                    observations = []
                    for position, index in enumerate(range(offset, min(offset + rows, len(AGENTS)))):
                        metrics = logo_pixels(image, left + 20, 189 + position * step + 18)
                        observations.append({"agent": AGENTS[index], **metrics})
                        seen.add(index)
                    pages.append({"file": path.name, "first_index": offset, "logos": observations})
                    signatures[offset] = row_signature(image)
                    return image

                offset = 0
                image = capture(offset, "page-01")
                first_signature = row_signature(image)
                while offset < final_offset:
                    old = row_signature(image)
                    xd("mousemove", "--window", window, left + content_width - 35, height - 67, "click", 1)
                    wait_for(lambda: row_signature(frame()) != old, "Next page did not change the Agent list", timeout=3)
                    offset = min(offset + rows, final_offset)
                    image = capture(offset, f"page-{len(pages) + 1:02d}")
                last_signature = row_signature(image)
                assert seen == set(range(len(AGENTS))), "Paging did not cover every fixture Agent"

                # At the end, one wheel step must expose precisely the preceding
                # row; compare rendered rows rather than OCR or a fixed text color.
                xd("mousemove", "--window", window, left + 8, 200, "click", 4)
                wait_for(lambda: row_signature(frame()) != last_signature, "Wheel-up did not move the list", timeout=3)
                previous = frame()
                if rows > 1:
                    assert row_signature(previous, 1) == last_signature, "One wheel step must move exactly one Agent"
                else:
                    assert row_signature(previous) == signatures[final_offset - 1], "One wheel step skipped an Agent"
                xd("click", 5)
                wait_for(lambda: row_signature(frame()) == last_signature, "Wheel-down did not return to the final Agent", timeout=3)
                frame().save(output / (name + "-last-by-scroll.png"))

                xd("click", "--repeat", len(AGENTS), "--delay", 15, 4)
                wait_for(lambda: row_signature(frame()) == first_signature, "Scrolling to the beginning lost the first Agent", timeout=3)
                xd("click", "--repeat", len(AGENTS), "--delay", 15, 5)
                wait_for(lambda: row_signature(frame()) == last_signature, "Scrolling to the end lost the last Agent", timeout=3)

                # Check the previous paging button and its forward round trip.
                xd("mousemove", "--window", window, left + content_width - 99, height - 67, "click", 1)
                wait_for(lambda: row_signature(frame()) != last_signature, "Previous page did not move the list", timeout=3)
                xd("mousemove", "--window", window, left + content_width - 35, height - 67, "click", 1)
                wait_for(lambda: row_signature(frame()) == last_signature, "Paging did not return to the last Agent", timeout=3)

                observed = [tuple(call[:call.index("--json")]) for call in calls()]
                assert len(observed) == 6 and set(observed) == READS, ("Navigation must use only the six initial reads", observed)
                subprocess.run(
                    ["xdotool", "key", "--window", window, "ctrl+q"],
                    check=False, stderr=subprocess.DEVNULL,
                )
                proc.wait(timeout=5)
                assert proc.returncode == 0, diagnostic.read_text(encoding="utf-8")
                return {
                    "case": name, "geometry": [width, height], "rows": rows,
                    "theme": theme, "language": language, "pages": pages,
                    "all_agent_logos_nonempty": True, "one_row_wheel_step": True,
                    "paging_reaches_last_agent": True, "scrolling_reaches_last_agent": True,
                    "read_only_calls": [list(command) for command in observed],
                    "renderer": diagnostic.read_text(encoding="utf-8").strip(),
                }
            except Exception:
                if "window" in locals() and proc.poll() is None:
                    x, y, actual_width, actual_height = geometry(window)
                    ImageGrab.grab(bbox=(x, y, x + actual_width, y + actual_height), xdisplay=display).save(output / (name + "-failure.png"))
                raise
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    try:
                        proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        proc.kill()
                        proc.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", nargs="?", default="target/release/zcall-gui")
    parser.add_argument("--output", default="_validation/agent-identities")
    args = parser.parse_args()
    display = os.environ.get("DISPLAY", "")
    if not display or display in (":0", ":0.0"):
        raise SystemExit("Use xvfb-run -a; host-display input is not allowed")
    binary = Path(args.binary).resolve()
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    results = []
    for case in CASES:
        result = run_case(binary, output, display, case)
        results.append(result)
        print(f"PASS {result['case']}: {len(result['pages'])} pages, {len(AGENTS)} Agent logos, six read-only CLI calls", flush=True)
    report = {
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "display": "private Xvfb", "data": "synthetic local CLI only",
        "host_input": False, "agents": AGENTS, "cases": results,
    }
    path = output / "agent-identities.json"
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
