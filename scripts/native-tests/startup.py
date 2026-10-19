#!/usr/bin/env python3
"""Verify the packaged application paints on isolated X11 or Wayland displays."""
import json
import os
from pathlib import Path
import re
import select
import subprocess
import sys
import tempfile
import time

from PIL import ImageGrab

binary = Path(sys.argv[-1]).resolve()
wayland = '--wayland' in sys.argv
collapsed = '--collapsed' in sys.argv
library = '--library' in sys.argv
shortcuts = '--shortcuts' in sys.argv
assert binary.is_file(), binary
if '--session' not in sys.argv:
    subprocess.run(['dbus-run-session', '--', sys.executable, __file__, '--session']
                   + (['--wayland'] if wayland else [])
                   + (['--collapsed'] if collapsed else [])
                   + (['--library'] if library else [])
                   + (['--shortcuts'] if shortcuts else []) + [str(binary)],
                   check=True, timeout=600 if library else 180 if shortcuts else 90)
    sys.exit(0)

with tempfile.TemporaryDirectory(prefix='feishin-startup-') as directory:
    root = Path(directory)
    if collapsed:
        (root / 'profile').mkdir()
        (root / 'profile/state.json').write_text(json.dumps({
            'settings': {'sidebar': {'collapsed': True, 'collapsedNavigation': True}}
        }))
    environment = dict(os.environ, XDG_RUNTIME_DIR=directory, XDG_SESSION_TYPE='x11',
                       FEISHIN_DATA_DIR=str(root / 'profile'), DISABLE_AUTO_UPDATES='1')
    environment.pop('WAYLAND_DISPLAY', None)
    environment.pop('WAYLAND_DEBUG', None)
    driver = next(iter(Path('/usr/share/vulkan/icd.d').glob('lvp*.json')), None)
    assert driver, 'The startup check requires software Vulkan'
    environment['VK_ICD_FILENAMES'] = str(driver)
    if list(Path('/usr/share/vulkan/explicit_layer.d').glob('*khronos*validation*.json')):
        environment['VK_INSTANCE_LAYERS'] = 'VK_LAYER_KHRONOS_validation'
    read_fd, write_fd = os.pipe()
    display = subprocess.Popen(['Xvfb', '-displayfd', str(write_fd), '-screen', '0',
                                '1440x1000x24', '-nolisten', 'tcp'], pass_fds=[write_fd],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os.close(write_fd)
    application = compositor = library_server = None
    suffix = ('-wayland' if wayland else '') + ('-collapsed' if collapsed else '') + ('-library' if library else '') + ('-shortcuts' if shortcuts else '')
    output = Path(os.environ.get('FEISHIN_STARTUP_OUTPUT', f'target/native-startup{suffix}.png'))
    output.parent.mkdir(parents=True, exist_ok=True)
    log_path = output.with_suffix('.log')
    try:
        assert select.select([read_fd], [], [], 10)[0], 'Virtual display did not start'
        number = os.read(read_fd, 32).decode().strip()
        assert number.isdigit(), number
        environment['DISPLAY'] = ':' + number
        if library or shortcuts:
            import library_workflow
            library_server = library_workflow.start(environment)
        with log_path.open('w') as log:
            if wayland:
                environment.update(XDG_SESSION_TYPE='wayland', WAYLAND_DISPLAY='feishin-startup')
                compositor = subprocess.Popen(
                    ['weston', '--backend=x11-backend.so', '--use-pixman', '--shell=kiosk-shell.so',
                     '--socket=feishin-startup', '--idle-time=0', '--width=1440', '--height=1000'],
                    env=environment, stdout=log, stderr=subprocess.STDOUT)
                deadline = time.monotonic() + 10
                while not (root / 'feishin-startup').exists():
                    assert compositor.poll() is None, log_path.read_text()
                    assert time.monotonic() < deadline, 'Isolated compositor did not start'
                    time.sleep(.05)
                environment['WAYLAND_DEBUG'] = '1'
            # D-Bus activates AT-SPI services with its own environment, which was
            # created before this private display and runtime directory existed.
            subprocess.run(['dbus-update-activation-environment', 'DISPLAY',
                            'XDG_RUNTIME_DIR', 'XDG_SESSION_TYPE']
                           + (['WAYLAND_DISPLAY'] if wayland else []),
                           env=environment, check=True, timeout=5)
            if library or shortcuts:
                workflow = library_workflow.Workflow(environment)
            application = subprocess.Popen([str(binary)], env=environment,
                                           stdout=log, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + 35
            while True:
                assert application.poll() is None, log_path.read_text()
                if wayland and not re.search(r'wl_surface@\d+\.attach\(wl_buffer@', log_path.read_text()):
                    assert time.monotonic() < deadline, 'Wayland window never presented a buffer'
                    time.sleep(.05)
                    continue
                # Weston has no PID property on some X11 backend versions.
                # Its class excludes the root window on this private display.
                window_query = ['--class', 'weston'] if wayland else ['--pid', str(application.pid)]
                windows = subprocess.run(['xdotool', 'search', '--onlyvisible', *window_query], env=environment,
                                         capture_output=True, text=True, timeout=5)
                presented = False
                for window in reversed(windows.stdout.splitlines()):
                    geometry = subprocess.run(['xdotool', 'getwindowgeometry', '--shell', window],
                                              env=environment, capture_output=True,
                                              text=True, timeout=5)
                    if geometry.returncode != 0:
                        continue
                    bounds = dict(line.split('=', 1) for line in geometry.stdout.splitlines())
                    x, y, width, height = (int(bounds[key]) for key in ['X', 'Y', 'WIDTH', 'HEIGHT'])
                    if width < 960 or height < 640:
                        continue
                    frame = ImageGrab.grab(xdisplay=environment['DISPLAY']).crop(
                        (x, y, x + width, y + height))
                    if frame.getcolors(maxcolors=32) is not None:
                        continue
                    frame.save(output)
                    presented = True
                    break
                if presented:
                    break
                assert time.monotonic() < deadline, 'Application never painted a window\n' + log_path.read_text()
                time.sleep(.05)
            subprocess.run(['xdotool', 'windowfocus', '--sync', window], env=environment,
                           check=True, timeout=5)
            if wayland:
                # X11 focus alone does not guarantee the kiosk client has focus.
                # Click the empty top edge, then wait for Wayland keyboard entry.
                subprocess.run(['xdotool', 'mousemove', '--window', window, str(width // 2), '8',
                                'click', '1'], env=environment, check=True, timeout=5)
                deadline = time.monotonic() + 5
                while True:
                    focus = re.findall(r'wl_keyboard@\d+\.(enter|leave)\(', log_path.read_text())
                    if focus and focus[-1] == 'enter':
                        break
                    assert time.monotonic() < deadline, 'Wayland client did not receive keyboard focus'
                    time.sleep(.05)
            if library or shortcuts:
                def capture(page):
                    time.sleep(.3)
                    ImageGrab.grab(xdisplay=environment['DISPLAY']).crop(
                        (x, y, x + width, y + height)).save(output.with_stem(output.stem + '-' + page))
                try:
                    if shortcuts:
                        workflow.click('Dismiss')
                        workflow.shortcut_settings(capture)
                    else:
                        workflow.run(capture)
                except Exception:
                    capture('failure')
                    raise
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'super+q'], env=environment,
                           check=True, timeout=5)
            assert application.wait(timeout=15) == 0, log_path.read_text()
        trace = log_path.read_text()
        assert not any(error in trace for error in
                       ['Cannot open Feishin', 'panicked at', 'VUID-', 'GPU hung']), trace
        assert not re.search(r'^\d+ (TRACE|DEBUG) \[', trace, re.M), 'Default logging leaked verbose records'
        if wayland:
            assert '.set_title("Feishin' in trace, trace
            assert not re.search(r'(protocol error|Error \d+ on .*xdg_|attached a buffer before configure)',
                                 trace, re.I), trace
        assert (root / 'profile').is_dir(), 'Application did not use the isolated profile'
        if collapsed:
            saved = json.loads((root / 'profile/state.json').read_text())
            assert saved['settings']['sidebar']['collapsed'], 'Collapsed sidebar setting was lost'
        print(f'Packaged app painted {width}x{height}, exited normally and used an isolated profile: {output}')
    finally:
        os.close(read_fd)
        if library_server is not None:
            library_server.shutdown()
            library_server.server_close()
        for process in [application, compositor, display]:
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
