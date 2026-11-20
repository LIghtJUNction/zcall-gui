#!/usr/bin/env python3
"""Native bootstrap UI against the offline helper fixture, on a private X server."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time
from types import SimpleNamespace

from PIL import ImageGrab

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("binary", nargs="?", default=str(ROOT / "target/release/zcall-gui"))
parser.add_argument("--output", default=str(ROOT / "_validation/core-install"))
args = parser.parse_args()
display = os.environ.get("DISPLAY", "")
if not display or display == ":0":
    raise SystemExit("Use a private xvfb-run display; host input is not allowed")
out = Path(args.output).resolve()
out.mkdir(parents=True, exist_ok=True)
spec = importlib.util.spec_from_file_location("bootstrap_fixture", ROOT / "scripts/test-core-install.py")
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)
fixture.ARGS = SimpleNamespace(binary=str(Path(args.binary).resolve(strict=True)))
fixture.BootstrapTests.setUpClass()
test = fixture.BootstrapTests("test_pending_exit_requires_explicit_completion")
test.setUp()
test.mode("pending")
env = dict(test.env, DISPLAY=display)
if "XAUTHORITY" in os.environ:
    env["XAUTHORITY"] = os.environ["XAUTHORITY"]
env.pop("OPENAI_API_KEY", None)
env.pop("ANTHROPIC_API_KEY", None)
process = None
window = ""


def xd(*values):
    return subprocess.check_output(["xdotool", *map(str, values)], text=True).strip()


def until(predicate, message, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process is not None and process.poll() is not None:
            raise AssertionError(process.stderr.read())
        if predicate():
            return
        time.sleep(0.06)
    raise AssertionError(message)


def click(x, y):
    xd("mousemove", "--window", window, x, y, "click", 1)
    time.sleep(0.15)


def shot(name):
    ImageGrab.grab(bbox=(0, 0, 1080, 740), xdisplay=display).save(out / (name + ".png"))


def launch(theme="light", page="overview", explicit_missing=True):
    global process, window
    select = ["--zcalld", "missing-fixture-zcalld"] if explicit_missing else []
    process = subprocess.Popen([fixture.ARGS.binary, *select,
                               "--theme", theme, "--language", "en", "--page", page,
                               "--smoke-seconds", "60"], env=env,
                              stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)

    def find():
        global window
        result = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(process.pid),
                                 "--name", "^zcall-gui"], capture_output=True, text=True)
        if result.returncode == 0:
            window = result.stdout.splitlines()[-1]
            return True
        return False

    until(find, "Native GUI window missing")
    xd("windowmove", "--sync", window, 0, 0)
    xd("windowfocus", "--sync", window)
    time.sleep(0.5)


def close():
    assert process is not None and process.poll() is None, "Owned GUI exited before its close action"
    sent = subprocess.run(["xdotool", "key", "--window", window, "ctrl+q"], capture_output=True, text=True)
    code = process.wait(timeout=5)
    assert code == 0, ("Owned GUI did not exit normally", code, process.stderr.read())
    # The press may close the window before xdotool sends the final key release.
    assert sent.returncode == 0 or "BadWindow" in sent.stderr, sent.stderr


try:
    launch()
    shot("missing-daemon-light")
    assert test.calls("curl.log") == 0, "Opening the GUI must not download an installer"
    click(300, 290)
    until(lambda: (test.cache / "latest.json").exists(), "Install action did not prepare a verified plan")
    time.sleep(0.3)
    assert not test.prefix.exists(), "Preparing must not install before confirmation"
    shot("install-confirm-light")
    click(558, 462)  # Cancel, leaving the reviewed local plan intact.
    assert not test.prefix.exists()
    click(960, 192)
    time.sleep(0.2)
    click(682, 462)
    until(lambda: test.saved()["last"].get("phase") == "pending_exit", "Pending result was not persisted")
    time.sleep(0.3)
    assert not test.prefix.exists()
    assert not test.config.exists(), "Pending must not remember an uninstalled binary"
    shot("waiting-for-exit-light")
    count = test.calls("curl.log")
    close()
    launch("dark", "updates")
    time.sleep(0.4)
    assert test.calls("curl.log") == count, "Reopening/status must be local-only"
    shot("waiting-for-exit-dark")
    click(960, 192)
    time.sleep(0.2)
    click(682, 462)
    until(lambda: test.config.exists() and "core_binary=" in test.config.read_text(encoding="utf-8"),
          "Committed result did not remember the actual binary")
    time.sleep(0.8)
    saved = test.saved()
    assert saved["last"]["phase"] == "committed"
    assert saved["last"]["committed"] is True
    assert test.calls("curl.log") == count
    shot("install-complete-dark")
    backend_calls = test.calls("helper.log")
    close()
    launch("dark", "updates", explicit_missing=False)
    until(lambda: test.calls("helper.log") >= backend_calls + 6, "Reopening did not discover the installed backend")
    time.sleep(0.35)
    shot("remembered-backend-dark")
    configuration = Path(env["XDG_CONFIG_HOME"]) / "zcalld/config.conf"
    unit = test.home / "fake-user-unit.json"
    running = test.home / "fake-service-running"
    assert not configuration.exists() and not unit.exists() and not running.exists()
    assert test.calls("init.log") == test.calls("service-install.log") == test.calls("service-start.log") == 0
    click(80, 142)
    shot("initialize-configuration-dark")
    click(300, 290)
    # A config created after the confirmation appeared must be preserved.
    configuration.parent.mkdir(parents=True, exist_ok=True)
    configuration.write_bytes(b"user-created-while-confirmation-open\n")
    before = test.calls("helper.log")
    click(682, 462)
    until(lambda: test.calls("helper.log") >= before + 2, "Setup did not re-read the committed installation and current configuration")
    time.sleep(0.3)
    assert configuration.read_bytes() == b"user-created-while-confirmation-open\n"
    assert test.calls("init.log") == 0, "Stale initialization must not reach init"
    assert not unit.exists() and not running.exists()
    shot("existing-configuration-preserved-dark")
    configuration.unlink()  # Only the synthetic file created above.
    before = test.calls("helper.log")
    click(1012, 713)
    until(lambda: test.calls("helper.log") >= before + 7, "Refresh did not finish")
    time.sleep(0.25)
    click(300, 290)
    shot("initialize-confirm-dark")
    before = test.calls("helper.log")
    click(682, 462)
    until(lambda: configuration.exists(), "Explicit initialization did not create the fixture config")
    until(lambda: test.calls("helper.log") >= before + 10, "Initialization did not refresh its state")
    time.sleep(0.25)
    assert configuration.read_bytes() == b"fixture starter\n"
    assert test.calls("init.log") == 1 and not unit.exists() and not running.exists()
    shot("install-user-service-dark")
    unit.write_text('{"owner":"unrelated-fixture"}\n', encoding="utf-8")
    foreign = unit.read_bytes()
    click(300, 290)
    click(682, 462)
    until(lambda: test.calls("service-install.log") == 1, "Explicit service verification was not invoked")
    time.sleep(0.3)
    assert unit.read_bytes() == foreign and not running.exists()
    assert test.calls("service-start.log") == 0
    shot("unrelated-user-service-preserved-dark")
    unit.unlink()  # Only this fixture's unrelated unit marker.
    before = test.calls("helper.log")
    click(300, 290)
    click(682, 462)
    until(lambda: test.calls("service-install.log") == 2, "Explicit service install retry was not invoked")
    until(lambda: test.calls("helper.log") >= before + 10, "Service installation did not refresh its state")
    time.sleep(0.25)
    assert unit.exists() and not running.exists() and test.calls("service-start.log") == 0
    shot("ready-to-start-dark")
    click(300, 290)
    click(682, 438)  # Existing service-start confirmation has the shorter height.
    until(lambda: running.exists(), "Explicit start did not reach the inert core fixture")
    time.sleep(0.5)
    assert test.calls("service-start.log") == 1 and test.calls("curl.log") == count
    shot("explicit-start-complete-dark")
    stat = Path(f"/proc/{process.pid}/stat")
    fields = stat.read_text().rsplit(")", 1)[1].split()
    ticks = sum(map(int, fields[11:13]))
    time.sleep(2)
    fields = stat.read_text().rsplit(")", 1)[1].split()
    idle = sum(map(int, fields[11:13])) - ticks
    report = {"scope": "private Xvfb; compiled inert curl/helper; synthetic installation",
              "default_open_network_requests": 0, "prepare_requires_confirmation": True,
              "cancel_keeps_plan_without_install": True, "pending_not_committed": True,
              "reopen_preserves_pending": True, "completion_remembers_binary": True,
              "reopen_discovers_backend_without_path_override": True,
              "setup_requires_each_explicit_confirmation": True,
              "config_created_after_confirmation_preserved": True,
              "unrelated_service_preserved_and_never_started": True,
              "fake_initialize_count": test.calls("init.log"),
              "fake_service_install_count": test.calls("service-install.log"),
              "fake_service_start_count": test.calls("service-start.log"),
              "actual_service_actions": 0, "no_actual_download_or_provider": True,
              "idle_cpu_ticks_2s": idle, "clock_ticks_per_second": os.sysconf("SC_CLK_TCK")}
    (out / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
finally:
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
    fixture.BootstrapTests.tearDownClass()
