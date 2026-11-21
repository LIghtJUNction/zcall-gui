#!/usr/bin/env python3
"""Exercise recording controls on an isolated native window and a stateful fake CLI."""
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


FIXTURE = r'''import json,pathlib,sys
root=pathlib.Path(__file__).parent
args=sys.argv[1:]
command=tuple(args[:args.index('--json')])
with (root/'calls.jsonl').open('a') as log:log.write(json.dumps(command)+'\n')
state=json.loads((root/'state.json').read_text())
def save():
    pending=root/'state.next'
    pending.write_text(json.dumps(state))
    pending.replace(root/'state.json')
if command in [('usage','enable'),('usage','disable')]:
    enabled=command[1]=='enable'
    changed=state['configured']!=enabled
    state['configured']=enabled
    save()
    result={'ok':True,'recording_configured':enabled,'restart_required':changed}
elif command==('service','restart'):
    state['restarts']+=1
    state['running_setting']=state['configured']
    save()
    result={'ok':True}
elif command==('usage','summary'):
    result=json.loads((root/'history.json').read_text())
    result['recording_configured']=state['configured']
else:
    responses={
        ('status',):{'version':'0.4.4','healthy':True,'listen':'127.0.0.1:3425'},
        ('providers','list'):{'providers':[]},
        ('agents','list'):{'agents':[]},
        ('plugins','list'):{'plugins':[]},
        ('privacy','show'):{'mode':'off','enabled':False},
        ('models',):{'object':'list','data':[]},
    }
    if command not in responses:
        print(json.dumps({'error':'Unexpected command in recording fixture'}));raise SystemExit(1)
    result=responses[command]
print(json.dumps(result))
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', nargs='?', default='target/release/zcall-gui')
    parser.add_argument('--output', default='_validation/usage-recording')
    parser.add_argument('--theme', choices=['light', 'dark'], default='dark')
    parser.add_argument('--language', default='en')
    options = parser.parse_args()
    display = os.environ.get('DISPLAY', '')
    if not display or display in (':0', ':0.0'):
        raise SystemExit('Run under xvfb-run -a; host-display input is not allowed')
    binary = pathlib.Path(options.binary).resolve()
    output = pathlib.Path(options.output).resolve()
    output.mkdir(parents=True, exist_ok=True)

    def xd(*args):
        return subprocess.check_output(['xdotool', *map(str, args)], text=True, timeout=5).strip()

    def windows():
        found = subprocess.run(
            ['xdotool', 'search', '--onlyvisible', '--name', '^zcall-gui'],
            text=True, capture_output=True, timeout=5,
        )
        return set(found.stdout.split()) if found.returncode == 0 else set()

    with tempfile.TemporaryDirectory(prefix='zcall-gui-recording-') as temporary:
        root = pathlib.Path(temporary)
        fixture = root / 'zcalld'
        fixture.write_text('#!' + sys.executable + '\n' + FIXTURE)
        fixture.chmod(0o755)
        state_file = root / 'state.json'
        state_file.write_text(json.dumps({'configured': False, 'running_setting': False, 'restarts': 0}))
        history_file = root / 'history.json'
        history_file.write_text(json.dumps({'requests': 0, 'failed': 0, 'input_tokens': 0, 'output_tokens': 0}))
        home = root / 'home'
        home.mkdir()
        environment = os.environ.copy()
        environment.update(
            HOME=str(home), XDG_CONFIG_HOME=str(home / '.config'),
            XDG_DATA_HOME=str(home / '.local/share'), XDG_STATE_HOME=str(home / '.local/state'),
        )
        diagnostic = output / 'renderer.log'
        before = windows()
        started = time.monotonic()
        with diagnostic.open('w') as errors:
            process = subprocess.Popen(
                [str(binary), '--zcalld', str(fixture), '--page', 'usage',
                 '--theme', options.theme, '--language', options.language, '--smoke-seconds', '45'],
                env=environment, stdout=subprocess.PIPE, stderr=errors,
            )

            def wait_for(predicate, message, timeout=8):
                deadline = min(started + 40, time.monotonic() + timeout)
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise AssertionError(message + ': ' + diagnostic.read_text())
                    if predicate():
                        return
                    time.sleep(.05)
                raise AssertionError(message)

            def state():
                return json.loads(state_file.read_text())

            def calls():
                path = root / 'calls.jsonl'
                return [tuple(json.loads(line)) for line in path.read_text().splitlines()] if path.exists() else []

            def count(*command):
                return calls().count(command)

            try:
                wait_for(lambda: len(windows() - before) == 1, 'Native fixture window missing')
                window = next(iter(windows() - before))
                xd('windowmove', '--sync', window, 0, 0)
                xd('windowfocus', '--sync', window)
                geometry = dict(
                    line.split('=', 1) for line in xd('getwindowgeometry', '--shell', window).splitlines()
                    if '=' in line
                )
                width, height = (int(geometry[key]) for key in ('WIDTH', 'HEIGHT'))
                assert 1000 <= width <= 1100 and height == 740, geometry
                content_width = min(1024, width - 230)
                left = 198 + (width - 230 - content_width) // 2

                def frame():
                    return ImageGrab.grab(bbox=(0, 0, width, height), xdisplay=display).convert('RGB')

                def capture(name):
                    frame().save(output / (name + '.png'))

                def click(x, y):
                    xd('mousemove', '--window', window, x, y, 'click', 1)
                    time.sleep(.15)

                def selected(page):
                    image = frame()
                    colors = [image.getpixel((13, 142 + row * 48)) for row in range(7)]
                    other = [color for row, color in enumerate(colors) if row != page]
                    return len(set(other)) == 1 and colors[page] != other[0]

                def confirm():
                    click(round((width - 452) / 2) + 368, round((height - 230) / 2) + 183)

                def refresh(expected):
                    click(left + content_width - 35, height - 23)
                    wait_for(lambda: count('usage', 'summary') == expected, 'Usage refresh was blocked')
                    time.sleep(.2)

                wait_for(lambda: 'Smoke ready: state loaded and presented' in diagnostic.read_text(),
                         'Initial usage snapshot was not rendered')
                assert count('usage', 'summary') == 1 and state()['restarts'] == 0
                capture('01-recording-disabled')

                # The visible empty-state primary action saves only the setting.
                click(left + 125, 294)
                wait_for(lambda: state()['configured'], 'Enable button did not save the recording setting')
                time.sleep(.2)
                assert count('usage', 'enable') == 1
                assert state()['running_setting'] is False and state()['restarts'] == 0
                capture('02-configured-pending-restart')

                # Opening and cancelling the confirmation must never restart.
                click(left + 125, 294)
                assert state()['restarts'] == 0 and count('service', 'restart') == 0
                capture('03-restart-confirmation')
                click(round((width - 452) / 2) + 244, round((height - 230) / 2) + 183)
                assert state()['restarts'] == 0
                click(left + 125, 294)
                assert state()['restarts'] == 0
                confirm()
                wait_for(lambda: state()['restarts'] == 1, 'Confirmed restart was not dispatched')
                wait_for(lambda: count('models') == 2, 'Post-restart snapshot did not finish')
                time.sleep(.2)
                assert state()['running_setting'] is True
                capture('04-configured-restart-cleared')

                # Once pending is cleared the same primary action connects tools,
                # instead of opening another restart dialog.
                click(left + 125, 294)
                wait_for(lambda: selected(2), 'Pending state did not clear after successful restart')
                assert state()['restarts'] == 1

                # Inject accounting fixture data independently of configuration.
                history_file.write_text(json.dumps({
                    'requests': 1250, 'failed': 5, 'input_tokens': 480000, 'output_tokens': 96000,
                    'cache_read_tokens': 160000, 'cache_write_tokens': 24000, 'reasoning_tokens': 18000,
                }))
                history_bytes = history_file.read_bytes()
                click(80, 382)
                wait_for(lambda: selected(5), 'Usage navigation failed')
                refresh(2)
                capture('05-full-metrics-configured')
                metrics = frame().crop((left, 154, left + content_width, 600)).tobytes()

                click(left + content_width - 84, 128)
                wait_for(lambda: not state()['configured'], 'Disable button did not save the setting')
                time.sleep(.2)
                assert count('usage', 'disable') == 1
                assert state()['running_setting'] is True and state()['restarts'] == 1
                assert history_file.read_bytes() == history_bytes
                assert frame().crop((left, 154, left + content_width, 600)).tobytes() == metrics
                capture('06-disabled-pending-history-retained')

                click(left + content_width - 258, 128)
                assert state()['restarts'] == 1
                confirm()
                wait_for(lambda: state()['restarts'] == 2, 'Disable restart was not dispatched')
                wait_for(lambda: count('models') == 3, 'Second post-restart snapshot did not finish')
                time.sleep(.2)
                assert state()['running_setting'] is False
                # The pending restart button is now absent. A click there must
                # not open a modal that blocks the next explicit refresh.
                click(left + content_width - 258, 128)
                refresh(3)
                assert history_file.read_bytes() == history_bytes
                assert frame().crop((left, 154, left + content_width, 600)).tobytes() == metrics
                assert state()['restarts'] == 2 and count('service', 'restart') == 2
                capture('07-disabled-restart-cleared-history-retained')

                allowed = {
                    ('status',), ('providers', 'list'), ('agents', 'list'), ('plugins', 'list'),
                    ('privacy', 'show'), ('models',), ('usage', 'summary'),
                    ('usage', 'enable'), ('usage', 'disable'), ('service', 'restart'),
                }
                assert set(calls()) <= allowed, calls()
                total_calls = len(calls())
                time.sleep(.3)
                assert len(calls()) == total_calls, 'Idle recording page invoked the CLI'
                # Quit may destroy the window before xdotool sends keyup.
                # Keep that diagnostic, but require the GUI's own clean exit.
                quit_input = subprocess.run(
                    ['xdotool', 'key', '--window', window, 'ctrl+q'],
                    text=True, capture_output=True, timeout=5,
                )
                process.communicate(timeout=5)
                final_diagnostic = diagnostic.read_text()
                assert process.returncode == 0, final_diagnostic
                assert 'Presented frames: ' in final_diagnostic, final_diagnostic
                result = {
                    'passed': True, 'theme': options.theme, 'language': options.language,
                    'window_pixels': [width, height], 'enabled_without_restart': True,
                    'restart_requires_confirmation': True, 'pending_cleared_after_restart': True,
                    'disabled_history_retained': True, 'service_restart_calls': 2,
                    'usage_summary_calls': 3, 'accounting_fixture_sha256': hashlib.sha256(history_bytes).hexdigest(),
                    'native_exit_code': process.returncode,
                    'quit_input_exit_code': quit_input.returncode,
                    'quit_input_stderr': quit_input.stderr.strip(),
                    'scope': 'Isolated native UI and stateful synthetic CLI; no service or provider was contacted.',
                }
                (output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
                print(json.dumps(result, indent=2))
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=3)


if __name__ == '__main__':
    main()
