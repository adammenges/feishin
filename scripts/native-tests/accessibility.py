#!/usr/bin/env python3
"""Verify the native AccessKit AT-SPI bridge on an isolated X11 display and D-Bus."""
import ast
import os
import pathlib
import re
import select
import subprocess
import sys
import tempfile
import time

binary = pathlib.Path(sys.argv[-1]).resolve()
assert binary.is_file(), binary
if '--session' not in sys.argv:
    subprocess.run(['dbus-run-session', '--', sys.executable, __file__, '--session', str(binary)], check=True, timeout=55)
    sys.exit(0)

with tempfile.TemporaryDirectory(prefix='feishin-atspi-') as directory:
    root = pathlib.Path(directory)
    environment = dict(os.environ, XDG_RUNTIME_DIR=directory, XDG_SESSION_TYPE='x11')
    environment.pop('WAYLAND_DISPLAY', None)
    environment.pop('WAYLAND_DEBUG', None)
    read_fd, write_fd = os.pipe()
    xvfb = subprocess.Popen(['Xvfb', '-displayfd', str(write_fd), '-screen', '0', '800x600x24', '-nolisten', 'tcp'], pass_fds=[write_fd], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os.close(write_fd)
    application = None
    monitor = None
    try:
        assert select.select([read_fd], [], [], 10)[0], 'Virtual display did not start'
        number = os.read(read_fd, 32).decode().strip()
        assert number.isdigit(), number
        environment['DISPLAY'] = ':' + number
        driver = next(iter(pathlib.Path('/usr/share/vulkan/icd.d').glob('lvp*.json')), None)
        if driver:
            environment['VK_ICD_FILENAMES'] = str(driver)
        def call(destination, path, method, *arguments, session=False):
            command = ['gdbus', 'call', '--session'] if session else ['gdbus', 'call', '--address', address]
            result = subprocess.run(command + ['--dest', destination, '--object-path', path, '--method', method, '--', *map(str, arguments)], env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5)
            assert result.returncode == 0, result.stderr
            return result.stdout.strip()
        address = ast.literal_eval(call('org.a11y.Bus', '/org/a11y/bus', 'org.a11y.Bus.GetAddress', session=True))[0]
        call('org.a11y.Bus', '/org/a11y/bus', 'org.freedesktop.DBus.Properties.Set', 'org.a11y.Status', 'IsEnabled', '<true>', session=True)
        call('org.a11y.Bus', '/org/a11y/bus', 'org.freedesktop.DBus.Properties.Set', 'org.a11y.Status', 'ScreenReaderEnabled', '<true>', session=True)
        log_path = pathlib.Path(os.environ.get('FEISHIN_ATSPI_TRACE', '/tmp/feishin-atspi-trace.log'))
        with log_path.open('w') as log:
            application = subprocess.Popen([str(binary)], env=environment, stdout=log, stderr=subprocess.STDOUT)
            accessible = 'org.a11y.atspi.Accessible'
            properties = 'org.freedesktop.DBus.Properties'
            objects = re.compile(r"\('([^']+)', (?:objectpath )?'([^']+)'\)")
            def children(destination, path):
                return objects.findall(call(destination, path, accessible + '.GetChildren'))
            def tree():
                found = {}
                pending = [('org.a11y.atspi.Registry', '/org/a11y/atspi/accessible/root')]
                seen = set()
                while pending:
                    destination, path = pending.pop()
                    if (destination, path) in seen:
                        continue
                    seen.add((destination, path))
                    name = call(destination, path, properties + '.Get', accessible, 'Name')
                    found.setdefault(name, (destination, path))
                    pending.extend(children(destination, path))
                return found
            deadline = time.monotonic() + 20
            while True:
                assert application.poll() is None, log_path.read_text()
                nodes = tree()
                volume = next((node for name, node in nodes.items() if 'Volume' in name), None)
                if volume:
                    break
                assert time.monotonic() < deadline, (nodes, log_path.read_text())
                time.sleep(.1)
            def named(label):
                return next(node for name, node in nodes.items() if name == "(<'" + label + "'>,)")
            assert not any('Hidden secret' in name or 'private-atspi-secret' in name for name in nodes)
            shuffle = named('Shuffle')
            disabled = named('Disabled action')
            username = named('Username')
            password = named('Password')
            event_path = root / 'events.log'
            with event_path.open('w') as event_log:
                monitor = subprocess.Popen(['stdbuf', '-oL', 'gdbus', 'monitor', '--address', address, '--dest', shuffle[0]], env=environment, stdout=event_log, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + 5
            while 'Monitoring signals' not in event_path.read_text():
                assert monitor.poll() is None and time.monotonic() < deadline, event_path.read_text()
                time.sleep(.05)
            assert '50.0' in call(*volume, properties + '.Get', 'org.a11y.atspi.Value', 'CurrentValue')
            assert 'true' in call(*shuffle, 'org.a11y.atspi.Action.DoAction', '0')
            while not re.search(r'Event.Object.Announcement.*Shuffle enabled', event_path.read_text()):
                assert time.monotonic() < deadline, event_path.read_text()
                time.sleep(.05)
            window_id = subprocess.check_output(['xdotool', 'search', '--name', '^Feishin AT-SPI fixture$'], env=environment, text=True).splitlines()[0]
            subprocess.run(['xdotool', 'windowfocus', '--sync', window_id], env=environment, check=True, timeout=5)
            assert 'true' in call(*username, 'org.a11y.atspi.Component.GrabFocus')
            deadline = time.monotonic() + 5
            while not (int(re.search(r"\[(?:uint32 )?(\d+)", call(*username, accessible + '.GetState'))[1]) & (1 << 12)):
                assert time.monotonic() < deadline, (call(*username, accessible + '.GetState'), log_path.read_text())
                time.sleep(.05)
            subprocess.run(['xdotool', 'type', '--clearmodifiers', '--delay', '1', 'atspi-user'], env=environment, check=True, timeout=5)
            call(*volume, properties + '.Set', 'org.a11y.atspi.Value', 'CurrentValue', '<73.0>')
            deadline = time.monotonic() + 5
            while '73.0' not in call(*volume, properties + '.Get', 'org.a11y.atspi.Value', 'CurrentValue'):
                assert time.monotonic() < deadline, log_path.read_text()
                time.sleep(.05)
            while 'atspi-user' not in call(*username, 'org.a11y.atspi.Text.GetText', '0', '-1'):
                assert time.monotonic() < deadline, log_path.read_text()
                time.sleep(.05)
            assert 'true' in call(*username, 'org.a11y.atspi.Text.SetCaretOffset', '3')
            while '3' not in call(*username, properties + '.Get', 'org.a11y.atspi.Text', 'CaretOffset'):
                assert time.monotonic() < deadline, log_path.read_text()
                time.sleep(.05)
            assert 'true' in call(*username, 'org.a11y.atspi.Text.AddSelection', '0', '5')
            deadline = time.monotonic() + 5
            while call(*username, 'org.a11y.atspi.Text.GetNSelections') != '(1,)':
                assert time.monotonic() < deadline, log_path.read_text()
                time.sleep(.05)
            assert call(*username, 'org.a11y.atspi.Text.GetSelection', '0') == '(0, 5)'
            assert 'true' in call(*username, 'org.a11y.atspi.Text.RemoveSelection', '0')
            assert 'private-atspi-secret' not in call(*password, 'org.a11y.atspi.Text.GetText', '0', '-1')
            assert 'true' in call(*named('Show password'), 'org.a11y.atspi.Action.DoAction', '0')
            deadline = time.monotonic() + 5
            while 'private-atspi-secret' not in call(*password, 'org.a11y.atspi.Text.GetText', '0', '-1'):
                assert time.monotonic() < deadline, log_path.read_text()
                time.sleep(.05)
            nodes = tree()
            assert 'true' in call(*named('Hide password'), 'org.a11y.atspi.Action.DoAction', '0')
            while 'private-atspi-secret' in call(*password, 'org.a11y.atspi.Text.GetText', '0', '-1'):
                assert time.monotonic() < deadline, log_path.read_text()
                time.sleep(.05)

            assert 'org.a11y.atspi.Action' not in call(*disabled, accessible + '.GetInterfaces')
            assert int(re.search(r'\[(?:uint32 )?(\d+)', call(*shuffle, accessible + '.GetState'))[1]) & (1 << 4)
            assert 'FEISHIN_ATSPI_SHUFFLE true' in log_path.read_text()
            assert 'FEISHIN_ATSPI_VOLUME 73' in log_path.read_text()
            print(f'Native AT-SPI: discovery, names, checkbox activation, live announcements, input focus/typing, text/caret/selection reading, slider values, disabled controls and password privacy passed; trace {log_path}')
    finally:
        os.close(read_fd)
        if monitor is not None:
            monitor.terminate()
            monitor.wait(timeout=5)
        if application is not None:
            application.terminate()
            try:
                application.wait(timeout=5)
            except subprocess.TimeoutExpired:
                application.kill()
                application.wait(timeout=5)
        xvfb.terminate()
        xvfb.wait(timeout=5)
