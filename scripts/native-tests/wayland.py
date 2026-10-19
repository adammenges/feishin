#!/usr/bin/env python3
"""Exercise retained GPUI windows in Weston on an isolated virtual display."""
import contextlib
import os
import pathlib
import re
import select
import shutil
import subprocess
import sys
import tempfile
import time


@contextlib.contextmanager
def running(arguments, log_path, **options):
    with log_path.open('w') as log:
        process = subprocess.Popen(arguments, stdout=log, stderr=subprocess.STDOUT, **options)
        try:
            yield process
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


binary = pathlib.Path(sys.argv[-1]).resolve()
assert binary.is_file(), binary
assert shutil.which('weston') and shutil.which('Xvfb'), 'Weston and a virtual X display are required'
with tempfile.TemporaryDirectory(prefix='feishin-wayland-') as directory:
    root = pathlib.Path(directory)
    environment = dict(os.environ, XDG_RUNTIME_DIR=directory, WAYLAND_DISPLAY='feishin-wayland', XDG_SESSION_TYPE='wayland')
    environment.pop('DISPLAY', None)
    read_fd, write_fd = os.pipe()
    try:
        with running(['Xvfb', '-displayfd', str(write_fd), '-screen', '0', '800x600x24', '-nolisten', 'tcp'],
                root / 'xvfb.log', pass_fds=[write_fd]):
            os.close(write_fd)
            write_fd = None
            assert select.select([read_fd], [], [], 10)[0], 'Virtual display did not start'
            number = os.read(read_fd, 32).decode().strip()
            assert number.isdigit(), (root / 'xvfb.log').read_text()
            environment['DISPLAY'] = ':' + number
            compositor_log = root / 'weston.log'
            with running(['weston', '--backend=x11-backend.so', '--use-pixman', '--shell=kiosk-shell.so',
                    '--socket=feishin-wayland', '--idle-time=0', '--width=800', '--height=600'],
                    compositor_log, env=environment) as compositor:
                deadline = time.monotonic() + 10
                while not (root / 'feishin-wayland').exists():
                    assert compositor.poll() is None, compositor_log.read_text()
                    assert time.monotonic() < deadline, 'Isolated compositor did not start'
                    time.sleep(.05)
                environment['WAYLAND_DEBUG'] = '1'
                driver = next(iter(pathlib.Path('/usr/share/vulkan/icd.d').glob('lvp*.json')), None)
                if driver:
                    environment['VK_ICD_FILENAMES'] = str(driver)
                result = subprocess.run([str(binary)], env=environment, stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT, text=True, timeout=30)
                trace = result.stdout
                output = pathlib.Path(os.environ.get('FEISHIN_WAYLAND_TRACE', '/tmp/feishin-wayland-window-trace.log'))
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_text(trace)
                assert result.returncode == 0, trace
                assert not re.search(r'(protocol error|Error \d+ on .*xdg_|attached a buffer before configure)', trace, re.I), trace
                mapped = re.search(r'wl_surface@(\d+)\.attach\(wl_buffer@', trace)
                assert mapped, 'Window never presented a buffer'
                assert 'FEISHIN_WAYLAND_PASSED' in trace, trace
                surface = rf'wl_surface@{mapped[1]}'
                buffer = rf'{surface}\.attach\(wl_buffer@'
                assert re.search(buffer, trace.split('FEISHIN_WAYLAND_HIDING 0', 1)[0]), 'Initial view did not present'
                for cycle in range(4):
                    hiding = trace.split(f'FEISHIN_WAYLAND_HIDING {cycle}', 1)[1].split(f'FEISHIN_WAYLAND_HIDDEN {cycle}', 1)[0]
                    assert len(re.findall(rf'{surface}\.attach\(nil, 0, 0\)', hiding)) == 1, hiding
                    assert re.search(rf'{surface}\.attach\(nil.*\n.*{surface}\.commit\(\)', hiding), hiding
                    hidden = trace.split(f'FEISHIN_WAYLAND_HIDDEN {cycle}', 1)[1].split(f'FEISHIN_WAYLAND_SHOWING {cycle}', 1)[0]
                    assert not re.search(rf'{surface}\.attach\(', hidden), hidden
                    next_marker = f'FEISHIN_WAYLAND_HIDING {cycle + 1}' if cycle < 3 else 'FEISHIN_WAYLAND_RAPID_START'
                    showing = trace.split(f'FEISHIN_WAYLAND_SHOWING {cycle}', 1)[1].split(next_marker, 1)[0]
                    first_buffer = re.search(buffer, showing)
                    assert first_buffer, f'Restored view {cycle} did not present'
                    setup = showing[:first_buffer.start()]
                    assert '.set_title("Feishin Wayland fixture")' in setup, setup
                    assert '.set_app_id("org.feishin.wayland-fixture")' in setup, setup
                    assert '.set_min_size(160, 120)' in setup, setup
                    commits = list(re.finditer(rf'{surface}\.commit\(\)', setup))
                    configure = re.search(r'xdg_surface@(\d+)\.configure\((\d+)\)', setup)
                    assert commits and configure and commits[0].start() < configure.start(), setup
                    acknowledge = re.search(rf'xdg_surface@{configure[1]}\.ack_configure\({configure[2]}\)', setup)
                    assert acknowledge and configure.start() < acknowledge.start(), setup
                rapid = trace.split('FEISHIN_WAYLAND_RAPID_START', 1)[1].split('FEISHIN_WAYLAND_RAPID_END', 1)[0]
                assert len(re.findall(rf'{surface}\.attach\(nil, 0, 0\)', rapid)) == 8, rapid
                assert re.search(buffer, trace.split('FEISHIN_WAYLAND_RAPID_END', 1)[1]), 'Rapid restore did not present'
                print(f'Wayland retained-window fixture: four hide/show cycles, hidden invalidation, configure/ack ordering, attributes, rapid restores and view preservation passed; trace {output}')
    finally:
        os.close(read_fd)
        if write_fd is not None:
            os.close(write_fd)
