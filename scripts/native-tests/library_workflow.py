"""Exercise the packaged app against a disposable library on startup.py's display."""
import ast
import importlib.util
import json
import math
import re
import shutil
import subprocess
import threading
import time
from http.server import ThreadingHTTPServer
from pathlib import Path


def start(environment):
    spec = importlib.util.spec_from_file_location(
        'native_fixture', Path(__file__).resolve().parents[1] / 'test-native-server.py')
    fixture = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixture)
    # Silence the source as well as discarding audio in MPV's null output.
    fixture.AUDIO = fixture.AUDIO[:44] + bytes(len(fixture.AUDIO) - 44)
    rich = json.loads((Path(__file__).resolve().parents[2] /
                       'crates/feishin-gpui/assets/mpris-reference.json').read_text())['metadata'][0]['song']
    details = {key: rich[key] for key in ['artists', 'albumArtists', 'genres', 'bpm',
                'comment', 'releaseDate', 'discNumber', 'trackNumber', 'playCount', 'userRating']}
    details['releaseDate'] = '2026-01-02'
    details['playDate'] = rich['lastPlayedAt']
    details['rating'] = rich['userRating']
    details['participants'] = {'artist': rich['artists'], 'albumartist': rich['albumArtists']}
    for song in [fixture.SONGS[1], fixture.TRACKS['playlist0'][1]]:
        song.update(details)
    if environment.get('FEISHIN_CHECK_PATH_SETTINGS') == '1':
        fixture.SONGS[0]['path'] = '/server/music/Track 001.wav'
    mpv = shutil.which('mpv')
    assert mpv, 'The packaged playback check requires MPV'
    profile = Path(environment['FEISHIN_DATA_DIR'])
    profile.mkdir(exist_ok=True)
    state_path = profile / 'state.json'
    state = json.loads(state_path.read_text()) if state_path.exists() else {}
    state.setdefault('settings', {}).update(
        mpv_path=mpv, volume=0., confirm_queue_changes=False,
        mpv_extra_args=['--no-config', '--ao=null', '--vo=null',
                        f'--log-file={profile / "mpv-null.log"}'])
    state_path.write_text(json.dumps(state))
    server = ThreadingHTTPServer(('127.0.0.1', 0), fixture.Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    environment.update(SERVER_TYPE='navidrome', SERVER_NAME='Isolated library',
                       SERVER_URL=f'http://127.0.0.1:{server.server_port}')
    return server


class Workflow:
    def __init__(self, environment):
        self.environment = environment
        self.address = ast.literal_eval(self.call(
            'org.a11y.Bus', '/org/a11y/bus', 'org.a11y.Bus.GetAddress', session=True))[0]
        for property_name in ['IsEnabled', 'ScreenReaderEnabled']:
            self.call('org.a11y.Bus', '/org/a11y/bus',
                      'org.freedesktop.DBus.Properties.Set', 'org.a11y.Status',
                      property_name, '<true>', session=True)

    def call(self, destination, path, method, *arguments, session=False):
        bus = ['--session'] if session else ['--address', self.address]
        result = subprocess.run(['gdbus', 'call', *bus, '--dest', destination,
                                 '--object-path', path, '--method', method, '--',
                                 *map(str, arguments)], env=self.environment,
                                capture_output=True, text=True, timeout=5)
        if result.returncode and (
                'org.freedesktop.DBus.Error.UnknownObject:' in result.stderr
                or ('org.freedesktop.DBus.Error.UnknownMethod:' in result.stderr
                    and 'Object does not exist at path' in result.stderr)):
            raise LookupError(result.stderr)
        assert result.returncode == 0, result.stderr
        return result.stdout.strip()

    def tree(self):
        pending = [('org.a11y.atspi.Registry', '/org/a11y/atspi/accessible/root')]
        seen = set()
        while pending:
            node = pending.pop()
            if node in seen:
                continue
            seen.add(node)
            try:
                name = self.call(*node, 'org.freedesktop.DBus.Properties.Get',
                                 'org.a11y.atspi.Accessible', 'Name')
                children = self.call(*node, 'org.a11y.atspi.Accessible.GetChildren')
            except LookupError:
                # Live notifications/navigation can remove a queued node.
                continue
            # Names are serialized as a GVariant string inside a tuple and variant.
            name = ast.literal_eval(name[2:-3])
            yield name, node
            pending.extend(re.findall(r"\('([^']+)', (?:objectpath )?'([^']+)'\)", children))

    def find(self, name, interface=None, role=None, last=False):
        deadline = time.monotonic() + 25
        while True:
            labels = []
            match = None
            try:
                for label, node in self.tree():
                    labels.append(label)
                    if (label == name and (interface is None or interface in self.call(
                            *node, 'org.a11y.atspi.Accessible.GetInterfaces'))
                            and (role is None or ast.literal_eval(self.call(
                                *node, 'org.a11y.atspi.Accessible.GetRole').replace('uint32 ', ''))[0] == role)):
                        if not last:
                            return node
                        match = node
                if match:
                    return match
            except LookupError:
                # Navigation replaced a node while its old tree was being read.
                pass
            assert time.monotonic() < deadline, (name, labels)
            time.sleep(.1)

    def click(self, name, role=None):
        print(f'Library check: activate {name}', flush=True)
        node = self.find(name, 'org.a11y.atspi.Action', role)
        assert 'true' in self.call(*node, 'org.a11y.atspi.Action.DoAction', '0'), name

    def type(self, name, text):
        print(f'Library check: fill {name}', flush=True)
        node = self.find(name, 'org.a11y.atspi.Text')
        assert 'true' in self.call(*node, 'org.a11y.atspi.Component.GrabFocus'), name
        deadline = time.monotonic() + 5
        while not (int(re.search(r"\[(?:uint32 )?(\d+)", self.call(
                *node, 'org.a11y.atspi.Accessible.GetState'))[1]) & (1 << 12)):
            assert time.monotonic() < deadline, f'Input did not receive focus: {name}'
            time.sleep(.05)
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'ctrl+a'],
                       env=self.environment, check=True, timeout=5)
        command = ['type', '--clearmodifiers', '--delay', '1', '--', text] if text else ['key', '--clearmodifiers', 'BackSpace']
        subprocess.run(['xdotool', *command],
                       env=self.environment, check=True, timeout=5)
        if name != 'Password':
            deadline = time.monotonic() + 5
            while ast.literal_eval(self.call(*node, 'org.a11y.atspi.Text.GetText', '0', '-1'))[0] != text:
                assert time.monotonic() < deadline, f'Input did not finish receiving text: {name}'
                time.sleep(.05)

    def progress(self, node):
        value = self.call(*node, 'org.freedesktop.DBus.Properties.Get',
                          'org.a11y.atspi.Value', 'CurrentValue')
        return float(value[2:-3])

    def wait_progress(self, node, expected):
        deadline = time.monotonic() + 15
        while True:
            value = self.progress(node)
            if expected(value):
                return value
            assert time.monotonic() < deadline, ('Playback progress', value)
            time.sleep(.1)

    def run(self, capture):
        self.click('Dismiss')
        self.find('Save Password', 'org.a11y.atspi.Action')
        self.click('✓ Remember credentials in OS password store')
        self.type('Username', 'test')
        self.type('Password', 'wrong')
        self.click('Sign in')
        self.find('Authentication expired. Sign in again.')
        self.type('Password', 'test')
        self.click('Sign in')
        self.find('Northern Lights')
        self.find('Test playlist')
        self.click('Dismiss')
        deadline = time.monotonic() + 10
        while any(label == 'Dismiss' for label, _ in self.tree()):
            assert time.monotonic() < deadline, 'Home message was not dismissed'
            time.sleep(.1)
        capture('home')
        self.click('Albums')
        self.find('Northern Lights')
        self.find('Night Signals')
        self.find('Fixture Artist', role=88)  # AT-SPI Link from default album credits.
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def album_state():
            state = json.loads((profile / 'state.json').read_text())
            key = f"{state['active_server']}:library:album"
            return state['settings']['page_sidebars']['album'], state['settings']['views'][key]['filters']
        assert album_state()[0], 'Default album filters were hidden'
        self.click('Is favorited: None')
        deadline = time.monotonic() + 5
        while album_state()[1]['favorite'] is not True:
            assert time.monotonic() < deadline, album_state()
            time.sleep(.1)
        self.click('Reset')
        deadline = time.monotonic() + 5
        while album_state()[1]['favorite'] is not None:
            assert time.monotonic() < deadline, album_state()
            time.sleep(.1)
        self.click('Unpin')
        deadline = time.monotonic() + 5
        while album_state()[0]:
            assert time.monotonic() < deadline, album_state()
            time.sleep(.1)
        self.click('Filters')
        self.click('Pin')
        deadline = time.monotonic() + 5
        while not album_state()[0]:
            assert time.monotonic() < deadline, album_state()
            time.sleep(.1)
        self.find('Northern Lights')
        print('Packaged filters: default visibility, immediate choice, Reset and saved pinning passed', flush=True)
        capture('albums')
        if self.environment.get('FEISHIN_CHECK_DETAIL_LINKS') == '1':
            self.click('Northern Lights', role=88)
            self.find('ALBUM')
            self.find('Track 001')
            capture('album')
            if self.environment.get('FEISHIN_CHECK_ALBUM_CONTROLS') == '1':
                self.album_controls(capture)
            self.click('Fixture Artist', role=88)
            self.find('VIEW DISCOGRAPHY')
            capture('artist')
            self.click('Albums')
            self.find('Northern Lights')
        self.click('Tracks')
        self.find('Track 001')
        capture('tracks')
        self.click('Test playlist')
        self.find('PLAYLIST')
        self.find('3 of 3 items')
        self.find('Track 001')
        capture('playlist')
        if self.environment.get('FEISHIN_CHECK_SMART_EDITOR') == '1':
            self.smart_editor(capture)
        self.click('Collapse')
        self.find('Track 003')
        capture('playlist-collapsed')
        self.click('Play')
        self.click('Play\nHold to shuffle')
        progress = self.find('Progress', 'org.a11y.atspi.Value')
        self.wait_progress(progress, lambda value: value > .5)
        self.click('Play / pause')
        time.sleep(1)
        paused = self.progress(progress)
        time.sleep(.8)
        assert abs(self.progress(progress) - paused) < .15, 'Playback did not pause'
        self.call(*progress, 'org.freedesktop.DBus.Properties.Set',
                  'org.a11y.atspi.Value', 'CurrentValue', '<20.0>')
        self.wait_progress(progress, lambda value: abs(value - 20.) < .5)
        self.click('Next track')
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        deadline = time.monotonic() + 15
        while True:
            state = json.loads((profile / 'state.json').read_text())
            queue = state['queue']
            current = next((entry for entry in queue['entries'] if entry['key'] == queue['current']), None)
            if current and current['item']['name'] == 'Track 002':
                break
            assert time.monotonic() < deadline, ('Next track', current)
            time.sleep(.1)
        assert len(queue['entries']) == 3
        assert state['active_server'] in state['signed_out_servers'], 'Credential retention was enabled'
        assert not next(server for server in state['servers']
                        if server['id'] == state['active_server'])['options']['save_password']
        assert 'AO: [null]' in (profile / 'mpv-null.log').read_text(), 'MPV did not use null output'
        self.click('Stop playback')
        self.wait_progress(progress, lambda value: value == 0.)
        time.sleep(1)
        assert self.progress(progress) == 0., 'Stopped progress changed after reset'
        self.click('Play / pause')
        self.wait_progress(progress, lambda value: .5 < value < 5.)
        self.click('Stop playback')
        self.wait_progress(progress, lambda value: value == 0.)
        self.media_controls(progress)
        capture('playback-stopped')
        self.click('View queue')
        self.click('Clear queue')
        self.wait_media('Metadata', lambda value: value == '(<@a{sv} {}>,)')
        self.click('View queue')
        self.shortcut_settings(capture)
        print('Packaged library: rejected login, sign-in, home, albums, tracks, playlist, '
              'silent MPV play/pause/seek/next/stop/restart and persisted queue passed')

    def smart_editor(self, capture):
        self.click('Create Playlist')
        raw_name = 'Navidrome smart playlist rules (JSON, optional)'
        original = {'all': [{'before': {'dateadded': '2021-05-15'}}],
                    'sort': '+dateAdded', 'limit': 20}
        self.type(raw_name, json.dumps(original))
        self.click('Visual rule editor')
        self.click('Date')
        self.click('May')
        self.find('2021-05-31', 'org.a11y.atspi.Action')
        capture('smart-calendar')
        self.click('2021-05-31')
        self.click('Edit JSON')

        def rules():
            node = self.find(raw_name, 'org.a11y.atspi.Text')
            return json.loads(ast.literal_eval(self.call(
                *node, 'org.a11y.atspi.Text.GetText', '0', '-1'))[0])

        expected = {**original, 'all': [{'before': {'dateadded': '2021-05-31'}}]}
        assert rules() == expected, rules()
        self.click('Visual rule editor')
        self.click('Clear filters')
        self.click('Reset to default')
        self.click('Edit JSON')
        assert rules() == expected, rules()
        self.click('Visual rule editor')
        self.click('Clear filters')
        self.click('Edit JSON')
        assert rules() == {**expected, 'all': []}, rules()
        capture('smart-cleared')
        self.click('Test playlist')
        self.find('3 of 3 items')
        print('Packaged smart editor: month-end calendar selection, Reset, Clear and JSON round trips passed', flush=True)

    def shortcut_settings(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        initial_zoom = json.loads((profile / 'state.json').read_text())['settings'].get('zoom', 1.)
        def binding():
            return json.loads((profile / 'state.json').read_text())['settings']['shortcuts']['next']
        self.click('Menu')
        self.click('Settings', role=35)  # AT-SPI MenuItem, distinct from player Settings.
        for label in ['General', 'Playback', 'Window', 'Advanced', 'Hotkeys']:
            self.click(label)
            deadline = time.monotonic() + 5
            while json.loads((profile / 'state.json').read_text())['settings']['settings_tab'] != label.lower():
                assert time.monotonic() < deadline, label
                time.sleep(.1)
            capture('settings-' + label.lower())
            if label == 'General' and self.environment.get('FEISHIN_CHECK_THEME_SETTINGS') == '1':
                self.theme_settings(capture)
            if label == 'General' and self.environment.get('FEISHIN_CHECK_IMAGE_SETTINGS') == '1':
                self.image_settings(capture)
            if label == 'General' and self.environment.get('FEISHIN_CHECK_PATH_SETTINGS') == '1':
                self.path_settings(capture)
            if label == 'Playback' and self.environment.get('FEISHIN_CHECK_COMPRESSOR_SETTINGS') == '1':
                self.compressor_settings(capture)
            if label == 'Playback' and self.environment.get('FEISHIN_CHECK_EQUALIZER_SETTINGS') == '1':
                self.equalizer_settings(capture)
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'ctrl+f'],
                       env=self.environment, check=True, timeout=5)
        self.type('Search settings', 'Keyboard shortcuts')
        self.click('Keyboard shortcuts')
        self.type('Search settings', 'Next track')
        self.click('Next track: Edit')
        self.find('…')
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'ctrl+j'],
                       env=self.environment, check=True, timeout=5)
        deadline = time.monotonic() + 5
        while binding()['key'] != 'ctrl-j':
            assert time.monotonic() < deadline, binding()
            time.sleep(.1)
        self.click('Next track: Edit')
        self.find('…')
        self.type('Search settings', 'Browser back')
        self.find('Browser back: Edit', 'org.a11y.atspi.Action')
        assert binding()['key'] == 'ctrl-j', 'Typing in search changed the recorded shortcut'
        self.type('Search settings', 'Next track')
        self.click('Next track: Clear')
        deadline = time.monotonic() + 5
        while binding() != {'key': '', 'global': False}:
            assert time.monotonic() < deadline, binding()
            time.sleep(.1)
        capture('shortcut-settings')
        print('Packaged shortcuts: search, record, focus cancellation, clear and persistence passed')
        self.physical_shortcuts(capture)
        self.duplicate_shortcuts(capture)
        # Restore the layout after testing Zoom in repeatedly. A full library run
        # also has side panels open, unlike the focused shortcut-only fixture.
        self.type('Search settings', 'Zoom out')
        self.click('Zoom out: Edit')
        self.find('…')
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'ctrl+minus'],
                       env=self.environment, check=True, timeout=5)
        def zoom():
            return json.loads((profile / 'state.json').read_text())['settings']['zoom']
        while zoom() > initial_zoom + .001:
            before = zoom()
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'ctrl+minus'],
                           env=self.environment, check=True, timeout=5)
            deadline = time.monotonic() + 5
            while abs(zoom() - before + .1) > .001:
                assert time.monotonic() < deadline, (before, zoom())
                time.sleep(.05)
        assert abs(zoom() - initial_zoom) < .001
        self.click('Zoom out: Clear')
        print('Packaged Zoom out: restored the starting zoom after shortcut checks')
        self.player_configuration(capture)

    def album_controls(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def state():
            return json.loads((profile / 'state.json').read_text())
        def saved(key, value):
            deadline = time.monotonic() + 5
            while True:
                current = state()
                view = current['settings']['views'][current['active_server'] + ':detail:albumDetail']
                if view[key] == value:
                    return
                assert time.monotonic() < deadline, (key, view[key], value)
                time.sleep(.1)
        queue = state()['queue']
        self.type('Search', 'zzzx_no_fixture_match')
        saved('text', 'zzzx_no_fixture_match')
        self.find('0 of 0 items')
        self.click('Clear')
        saved('text', '')
        self.find('Track 001')
        self.click('ID')
        self.click('Name', role=35)
        saved('sort', 'Name')
        self.click('Sort order')
        saved('descending', True)
        self.click('Sort order')
        saved('descending', False)
        self.click('Name')
        self.click('ID', role=35)
        saved('sort', 'Id')
        self.click('List layout')
        self.find('Configure')
        self.click('Save')
        self.find('Track 001')
        assert state()['queue'] == queue
        capture('album-controls')
        print('Packaged album controls: search/clear, sorting, layout and queue preservation passed', flush=True)

    def theme_settings(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def settings():
            return json.loads((profile / 'state.json').read_text())['settings']
        def saved(key, value):
            deadline = time.monotonic() + 5
            while settings()[key] != value:
                assert time.monotonic() < deadline, (key, settings()[key], value)
                time.sleep(.1)
        def enter():
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Return'],
                           env=self.environment, check=True, timeout=5)
        self.click('Theme')
        self.type('Theme', 'Dracula')
        enter()
        saved('theme', 'dracula')
        self.click('Theme')
        self.type('Theme', 'Default Dark')
        self.click('Default Dark')
        saved('theme', 'defaultDark')
        self.click('Use system theme')
        saved('follow_system_theme', True)
        self.find('Theme (dark)', 'org.a11y.atspi.Action')
        self.find('Theme (light)', 'org.a11y.atspi.Action')
        self.click('Use system theme')
        saved('follow_system_theme', False)
        # Custom accent is enabled by default. Exercise both transitions before editing.
        if not settings()['use_theme_accent_color']:
            self.click('Use theme accent color')
            saved('use_theme_accent_color', True)
        self.click('Use theme accent color')
        saved('use_theme_accent_color', False)
        self.type('Accent color', 'rgb(29, 185, 84)')
        enter()
        saved('accent', 'rgb(29, 185, 84)')
        if self.environment.get('FEISHIN_CHECK_ACCENT_SPECTRUM') == '1':
            self.click('Accent color', role=43)
            hue = self.find('Hue', 'org.a11y.atspi.Value')
            self.call(*hue, 'org.freedesktop.DBus.Properties.Set',
                      'org.a11y.atspi.Value', 'CurrentValue', '<180.0>')
            saved('accent', 'rgb(29, 185, 185)')
            saturation = self.find('Color saturation', 'org.a11y.atspi.Value')
            if self.environment['XDG_SESSION_TYPE'] == 'x11':
                previous = None
                deadline = time.monotonic() + 5
                stable_since = time.monotonic()
                while True:
                    extents = ast.literal_eval(self.call(
                        *saturation, 'org.a11y.atspi.Component.GetExtents', '0'))[0]
                    now = time.monotonic()
                    if extents != previous:
                        previous, stable_since = extents, now
                    elif now - stable_since >= .2:
                        break
                    assert now < deadline, 'Saturation bounds did not settle'
                    time.sleep(.05)
                x, y, width, height = extents
                print(f'Packaged saturation bounds: {extents}; window {self.call(*saturation, "org.a11y.atspi.Component.GetExtents", "1")}', flush=True)
                capture('accent-ready')
                subprocess.run(['xdotool', 'mousemove', str(x + width // 2),
                    str(y + height // 2), 'mousedown', '1'],
                    env=self.environment, check=True, timeout=5)
                try:
                    entry = self.find('Accent color', 'org.a11y.atspi.Text')
                    deadline = time.monotonic() + 5
                    while 'rgb(64, 128, 128)' not in (preview := self.call(*entry, 'org.a11y.atspi.Text.GetText', '0', '-1')):
                        assert time.monotonic() < deadline, f'Saturation drag did not preview its center color: {preview}'
                        time.sleep(.05)
                    assert settings()['accent'] == 'rgb(29, 185, 185)', 'Saturation drag saved before release'
                    capture('accent-spectrum')
                finally:
                    subprocess.run(['xdotool', 'mouseup', '1'], env=self.environment, check=True, timeout=5)
                saved('accent', 'rgb(64, 128, 128)')
            else:
                self.call(*saturation, 'org.freedesktop.DBus.Properties.Set',
                    'org.a11y.atspi.Value', 'CurrentValue', '<0.0>')
                saved('accent', 'rgb(185, 185, 185)')
                capture('accent-spectrum')
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Escape'], env=self.environment, check=True, timeout=5)
            self.type('Accent color', 'rgb(29, 185, 84)')
            enter()
            saved('accent', 'rgb(29, 185, 84)')
            print('Packaged accent spectrum: hue, saturation, preview/commit and Escape passed')
        self.click('Use theme accent color')
        saved('use_theme_accent_color', True)
        self.click('Use theme primary shade')
        shade = self.find('Primary shade', 'org.a11y.atspi.Value')
        self.call(*shade, 'org.freedesktop.DBus.Properties.Set',
                  'org.a11y.atspi.Value', 'CurrentValue', '<8.0>')
        saved('primary_shade', 8)
        self.click('Use theme primary shade')
        saved('use_theme_primary_shade', True)
        capture('theme-settings')
        themes = profile / 'Themes'
        themes.mkdir(exist_ok=True)
        custom = themes / 'native-warning.json'
        custom.write_text(json.dumps({'extends': 'defaultDark',
            'colors': {'primary': 17, 'surface': '#24364a'}}))
        self.click('Reload')
        self.find('native-warning: Ignored invalid color value(s) for: primary')
        self.click('Theme')
        self.type('Theme', 'Native Warning')
        self.click('Native Warning')
        saved('theme', 'native-warning')
        capture('theme-warning')
        custom.write_text(json.dumps({'extends': 'defaultDark',
            'colors': {'primary': '#abcdef', 'surface': '#24364a'}}))
        self.click('Reload')
        deadline = time.monotonic() + 5
        while any('Ignored invalid color value(s)' in label for label, _ in self.tree()):
            assert time.monotonic() < deadline, 'Corrected custom theme kept its warning'
            time.sleep(.1)
        self.click('Theme')
        self.type('Theme', 'Default Dark')
        self.click('Default Dark')
        saved('theme', 'defaultDark')
        custom.unlink()
        self.click('Reload')
        print('Packaged themes: keyboard selection, system choices, RGB, shade, invalid-color recovery and persistence passed')
        if self.environment.get('FEISHIN_CHECK_INLINE_CSS') == '1':
            # Original local-search shortcuts intentionally ignore focused form fields.
            self.click('Search', role=43)
            self.type('Search settings', 'Custom CSS')
            self.click('Enable custom CSS')
            self.click('Edit')
            draft = '.fs-player-bar-module-container { border-radius: 3px; }'
            self.type('Custom CSS', draft)
            assert settings()['css']['content'] == '', 'Typing applied an unsaved CSS draft'
            self.click('Close')
            assert settings()['css']['content'] == '', 'Closing applied an unsaved CSS draft'
            self.click('Edit')
            entry = self.find('Custom CSS', 'org.a11y.atspi.Text')
            assert draft in self.call(*entry, 'org.a11y.atspi.Text.GetText', '0', '-1')
            self.click('Save')
            saved('css', {'enabled': True, 'content': draft})
            assert (profile / 'custom.css').read_text() == draft
            capture('inline-css')
            external = '.fs-player-bar-module-container { border-radius: 4px; }'
            (profile / 'custom.css').write_text(external)
            saved('css', {'enabled': True, 'content': external})
            assert draft in self.call(*entry, 'org.a11y.atspi.Text.GetText', '0', '-1'), 'External change replaced the active editor'
            self.click('Close')
            self.click('Enable custom CSS')
            saved('css', {'enabled': False, 'content': external})
            self.type('Search settings', '')
            print('Packaged inline CSS: draft retention, Save, file reload and disabling passed')

    def image_settings(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def settings():
            return json.loads((profile / 'state.json').read_text())['settings']['image_resolution']
        def saved(key, expected):
            deadline = time.monotonic() + 5
            while settings()[key] != expected:
                assert time.monotonic() < deadline, (key, settings(), expected)
                time.sleep(.1)
        if not any(label == 'Search settings' for label, _ in self.tree()):
            self.click('Search', role=43)
        self.type('Search settings', 'Image resolution')
        self.click('Edit')
        self.type('Table Image resolution', '123')
        saved('table', 123)
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Up'], env=self.environment, check=True, timeout=5)
        saved('table', 124)
        self.type('Table Image resolution', '')
        saved('table', 124)
        self.click('Close')
        self.click('Edit')
        entry = self.find('Table Image resolution', 'org.a11y.atspi.Text')
        assert ast.literal_eval(self.call(*entry, 'org.a11y.atspi.Text.GetText', '0', '-1'))[0] == '124'
        self.type('Sidebar Image resolution', '2001')
        saved('sidebar', 2001)
        self.click('Close')
        saved('sidebar', 2000)
        self.click('Edit')
        self.type('Table Image resolution', '123.5')
        saved('table', 123.5)
        self.type('Header Image resolution', '-1')
        saved('header', -1)
        self.click('Close')
        saved('header', 0)
        self.click('Edit')
        entry = self.find('Table Image resolution', 'org.a11y.atspi.Text')
        assert ast.literal_eval(self.call(*entry, 'org.a11y.atspi.Text.GetText', '0', '-1'))[0] == '123.5'
        capture('image-settings')
        self.type('Table Image resolution', '80')
        self.type('Sidebar Image resolution', '400')
        self.type('Header Image resolution', '300')
        saved('table', 80)
        saved('sidebar', 400)
        self.click('Close')
        self.type('Search settings', '')
        print('Packaged image settings: inline editing, fractional values, immediate persistence, keyboard steps, empty drafts and blur clamping passed', flush=True)

    def path_settings(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def saved(key, expected):
            deadline = time.monotonic() + 5
            while json.loads((profile / 'state.json').read_text())['settings'][key] != expected:
                assert time.monotonic() < deadline, (key, expected)
                time.sleep(.1)
        if not any(label == 'Search settings' for label, _ in self.tree()):
            self.click('Search', role=43)
        self.type('Search settings', 'File path replacement')
        self.find('/server/music/Track 001.wav')
        self.type('Remove prefix', '/server/music/')
        saved('path_replace', '/server/music/')
        self.find('Track 001.wav')
        self.type('Add prefix', '/local/')
        saved('path_replace_with', '/local/')
        self.find('/local/Track 001.wav')
        self.click('Refresh')
        self.find('/local/Track 001.wav')
        capture('path-settings')
        self.type('Remove prefix', '')
        self.type('Add prefix', '')
        saved('path_replace', '')
        saved('path_replace_with', '')
        self.find('/server/music/Track 001.wav')
        self.type('Search settings', '')
        print('Packaged paths: delayed persistence, mapped preview, refresh and reset passed', flush=True)

    def compressor_settings(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def saved(key, expected):
            deadline = time.monotonic() + 5
            while True:
                actual = json.loads((profile / 'state.json').read_text())['settings']['compressor'][key]
                # Native DSP values are f32, promoted to JSON numbers during save.
                if actual == expected or (isinstance(expected, float) and
                        math.isclose(actual, expected, rel_tol=1e-7, abs_tol=1e-8)):
                    return
                assert time.monotonic() < deadline, (key, expected, actual)
                time.sleep(.1)
        if not any(label == 'Search settings' for label, _ in self.tree()):
            self.click('Search', role=43)
        self.type('Search settings', 'Compressor')
        self.click('Compressor')
        saved('enabled', True)
        for label, key, value in [('Makeup Gain', 'makeup', 30), ('Knee', 'knee', 10)]:
            slider = self.find(label, 'org.a11y.atspi.Value')
            self.call(*slider, 'org.freedesktop.DBus.Properties.Set',
                      'org.a11y.atspi.Value', 'CurrentValue', f'<{value}.0>')
            saved(key, value)
            self.wait_progress(slider, lambda current: current == value)
        self.type('Attack', '0.1')
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Return'],
                       env=self.environment, check=True, timeout=5)
        saved('attack', .1)
        self.type('Makeup Gain', '35')
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Return'],
                       env=self.environment, check=True, timeout=5)
        saved('makeup', 30)
        capture('compressor-settings')
        self.click('Reset to default')
        saved('attack', 20)
        saved('makeup', 6)
        saved('knee', 2.83)
        self.click('Compressor')
        saved('enabled', False)
        self.type('Search settings', '')
        print('Packaged compressor: slider limits, typed decimals, clamping, reset and enable state passed', flush=True)

    def equalizer_settings(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def settings():
            return json.loads((profile / 'state.json').read_text())['settings']
        def saved(predicate):
            deadline = time.monotonic() + 5
            while not predicate(settings()):
                assert time.monotonic() < deadline, settings()['equalizer']
                time.sleep(.1)
        if not any(label == 'Search settings' for label, _ in self.tree()):
            self.click('Search', role=43)
        self.type('Search settings', 'Equalizer')
        self.click('Equalizer')
        saved(lambda state: state['equalizer_enabled'])
        self.type('31.5 Hz', '3.25')
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Return'],
                       env=self.environment, check=True, timeout=5)
        saved(lambda state: state['equalizer'][0] == 3.25)
        preamp = self.find('Preamp', 'org.a11y.atspi.Value')
        self.call(*preamp, 'org.freedesktop.DBus.Properties.Set',
                  'org.a11y.atspi.Value', 'CurrentValue', '<5.5>')
        saved(lambda state: state['equalizer_preamp'] == 5.5)
        self.type('Preset name...', 'Workflow EQ')
        subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Return'],
                       env=self.environment, check=True, timeout=5)
        saved(lambda state: state['equalizer_presets'].get('Workflow EQ', [None])[0] == 3.25)
        self.click('Reset')
        saved(lambda state: state['equalizer'] == [0] * 12 and state['equalizer_preamp'] == 0)
        self.click('Select preset')
        self.type('Search', 'Workflow EQ')
        self.click('Workflow EQ')
        saved(lambda state: state['equalizer'][0] == 3.25)
        capture('equalizer-settings')
        self.click('Delete custom...')
        self.click('Workflow EQ')
        saved(lambda state: 'Workflow EQ' not in state['equalizer_presets'])
        self.click('Reset')
        self.click('Equalizer')
        saved(lambda state: not state['equalizer_enabled'])
        self.type('Search settings', '')
        print('Packaged equalizer: band numbers, preamp, preset save/load/delete, reset and enable state passed', flush=True)

    def duplicate_shortcuts(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def state():
            return json.loads((profile / 'state.json').read_text())
        def key(stroke):
            subprocess.run(['xdotool', 'key', '--clearmodifiers', stroke],
                           env=self.environment, check=True, timeout=5)
        for label, action in [('Zoom in', 'zoomIn'), ('Toggle repeat', 'toggleRepeat')]:
            self.type('Search settings', label)
            self.click(label + ': Edit')
            self.find('…')
            key('ctrl+j')
            deadline = time.monotonic() + 5
            while state()['settings']['shortcuts'][action]['key'] != 'ctrl-j':
                assert time.monotonic() < deadline, state()['settings']['shortcuts'][action]
                time.sleep(.1)
        before = state()
        key('ctrl+j')
        deadline = time.monotonic() + 5
        expected = {'Off': 'All', 'All': 'One', 'One': 'Off'}[before['queue']['repeat']]
        while abs(state()['settings']['zoom'] - before['settings']['zoom'] - .1) > .001 or state()['queue']['repeat'] != expected:
            assert time.monotonic() < deadline, (state()['settings']['zoom'], state()['queue']['repeat'])
            time.sleep(.1)
        self.type('Search settings', 'Toggle repeat')
        key('ctrl+j')
        time.sleep(.3)
        assert state()['queue']['repeat'] == expected, 'Duplicate shortcuts fired inside search'
        assert abs(state()['settings']['zoom'] - before['settings']['zoom'] - .1) < .001
        capture('duplicate-shortcuts')
        self.click('Toggle repeat: Clear')
        self.type('Search settings', 'Zoom in')
        self.click('Zoom in: Clear')
        print('Packaged duplicate shortcuts: recording, both actions, input exclusion and clearing passed')

    def physical_shortcuts(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def settings():
            return json.loads((profile / 'state.json').read_text())['settings']
        def key(stroke):
            subprocess.run(['xdotool', 'key', '--clearmodifiers', stroke],
                           env=self.environment, check=True, timeout=5)
        def record_and_dispatch(stroke, expected, other):
            self.click('Zoom in: Edit')
            self.find('…')
            key(stroke)
            deadline = time.monotonic() + 5
            while settings()['shortcuts']['zoomIn'] != {
                    'key': expected, 'global': False, 'physical': True}:
                assert time.monotonic() < deadline, (stroke, settings()['shortcuts']['zoomIn'])
                time.sleep(.1)
            before = settings()['zoom']
            key(other)
            time.sleep(.3)
            assert settings()['zoom'] == before, (other, 'Matched a different physical key')
            key(stroke)
            deadline = time.monotonic() + 5
            while abs(settings()['zoom'] - before - .1) > .001:
                assert time.monotonic() < deadline, (stroke, before, settings()['zoom'])
                time.sleep(.1)
        self.type('Search settings', 'Zoom in')
        for stroke, expected, other in [
                ('ctrl+shift+equal', 'ctrl-shift-=', 'ctrl+equal'),
                ('ctrl+KP_Add', 'ctrl-numpadadd', 'ctrl+shift+equal'),
                ('ctrl+KP_Enter', 'ctrl-numpadenter', 'ctrl+Return'),
                ('ctrl+KP_1', 'ctrl-numpad1', 'ctrl+1')]:
            record_and_dispatch(stroke, expected, other)
        if self.environment['XDG_SESSION_TYPE'] == 'x11':
            # This X server belongs to startup.py, never the user's desktop.
            subprocess.run(['setxkbmap', '-layout', 'de'], env=self.environment,
                           check=True, timeout=5)
            try:
                mapping = subprocess.run(['setxkbmap', '-query'], env=self.environment,
                                         check=True, capture_output=True, text=True, timeout=5)
                assert re.search(r'layout:\s+de', mapping.stdout), mapping.stdout
                record_and_dispatch('ctrl+y', 'ctrl-z', 'ctrl+z')
            finally:
                subprocess.run(['setxkbmap', '-layout', 'us'], env=self.environment,
                               check=True, timeout=5)
        capture('physical-shortcuts')
        self.click('Zoom in: Clear')
        print('Packaged physical shortcuts: shifted symbols, distinct keypad keys, saved recording and action dispatch passed')

    def player_configuration(self, capture):
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        def settings():
            return json.loads((profile / 'state.json').read_text())['settings']
        def press(name, last=False):
            node = self.find(name, 'org.a11y.atspi.Action', last=last)
            if self.environment['XDG_SESSION_TYPE'] == 'wayland':
                assert 'true' in self.call(*node, 'org.a11y.atspi.Action.DoAction', '0'), name
                return
            # Coordinates belong to startup.py's private X server.
            # Search and expanded menus can move a control after it first appears.
            # Wait for its geometry, not a guessed delay after the previous action.
            previous = None
            deadline = time.monotonic() + 5
            stable_since = time.monotonic()
            while True:
                extents = ast.literal_eval(self.call(
                    *node, 'org.a11y.atspi.Component.GetExtents', '0'))[0]
                now = time.monotonic()
                if extents != previous:
                    previous, stable_since = extents, now
                elif now - stable_since >= .2:
                    break
                assert now < deadline, f'Control bounds did not settle: {name}'
                time.sleep(.05)
            x, y, width, height = extents
            print(f'Library check: press {name} at {(x, y, width, height)}', flush=True)
            # Repeated menu clicks can leave the pointer at this exact position;
            # --sync would wait forever for a motion event that never occurs.
            subprocess.run(['xdotool', 'mousemove',
                            str(x + width // 2), str(y + height // 2), 'click', '1'],
                           env=self.environment, check=True, timeout=5)
            time.sleep(.2)
        def escape():
            subprocess.run(['xdotool', 'key', '--clearmodifiers', 'Escape'],
                           env=self.environment, check=True, timeout=5)
            time.sleep(.2)
        self.click('Menu')
        self.click('Settings', role=35)
        self.click('Playback')
        self.type('Search settings', 'Crossfade style')
        # The Configure button uses its Settings tooltip as its accessible name.
        # The reverse tree walk reaches the player bar before the Settings page.
        press('Settings', last=True)
        press('MPV')
        self.click('Native', role=35)
        press('Crossfade')
        deadline = time.monotonic() + 5
        while settings()['transition'] != 'crossfade':
            assert time.monotonic() < deadline, settings()['transition']
            time.sleep(.1)
        assert settings()['mpv_path'] == ''
        press('Equal Power')
        self.find('Linear', 'org.a11y.atspi.Action', role=35)
        escape()
        press('Equal Power')
        self.click('Linear', role=35)
        deadline = time.monotonic() + 5
        while settings()['crossfade_style'] != 'linear':
            assert time.monotonic() < deadline, settings()['crossfade_style']
            time.sleep(.1)
        capture('player-configuration')
        escape()
        for name, node in self.tree():
            if name == 'Settings':
                role = ast.literal_eval(self.call(*node, 'org.a11y.atspi.Accessible.GetRole').replace('uint32 ', ''))[0]
                assert role != 16, 'Escape left the player configuration dialog open'
        print('Packaged player configuration: scrolling Settings, nested menus, saved transitions and Escape passed')

    def mpris_get(self, name):
        return self.call('org.mpris.MediaPlayer2.Feishin', '/org/mpris/MediaPlayer2',
                         'org.freedesktop.DBus.Properties.Get',
                         'org.mpris.MediaPlayer2.Player', name, session=True)

    def mpris_set(self, name, value):
        return self.call('org.mpris.MediaPlayer2.Feishin', '/org/mpris/MediaPlayer2',
                         'org.freedesktop.DBus.Properties.Set',
                         'org.mpris.MediaPlayer2.Player', name, value, session=True)

    def mpris_command(self, name, *arguments):
        return self.call('org.mpris.MediaPlayer2.Feishin', '/org/mpris/MediaPlayer2',
                         'org.mpris.MediaPlayer2.Player.' + name, *arguments, session=True)

    def wait_media(self, name, expected):
        deadline = time.monotonic() + 15
        while True:
            value = self.mpris_get(name)
            if expected(value):
                return value
            assert time.monotonic() < deadline, (name, value)
            time.sleep(.1)

    def media_controls(self, progress):
        print('Library check: desktop media controls', flush=True)
        profile = Path(self.environment['FEISHIN_DATA_DIR'])
        # These reads fail on the original Rust bridge, which omitted both properties.
        assert self.mpris_get('Shuffle') == '(<false>,)'
        assert self.mpris_get('LoopStatus') == "(<'None'>,)"
        current = json.loads((profile / 'state.json').read_text())['queue']['current']
        for enabled in ['true', 'true', 'false', 'false']:
            self.mpris_set('Shuffle', '<' + enabled + '>')
            self.wait_media('Shuffle', lambda value: value == '(<' + enabled + '>,)')
            state = json.loads((profile / 'state.json').read_text())
            assert state['queue']['shuffle'] == (enabled == 'true')
            assert state['queue']['current'] == current
        for status, repeat in [('Track', 'One'), ('Track', 'One'), ('Playlist', 'All'), ('None', 'Off')]:
            self.mpris_set('LoopStatus', "<'" + status + "'>")
            self.wait_media('LoopStatus', lambda value: value == "(<'" + status + "'>,)")
            assert json.loads((profile / 'state.json').read_text())['queue']['repeat'] == repeat
        for requested, expected in [(.374, .37), (-.1, 0.), (1.5, 1.), (0., 0.)]:
            self.mpris_set('Volume', '<' + str(requested) + '>')
            self.wait_media('Volume', lambda value: abs(float(value[2:-3]) - expected) < .0001)
        metadata = self.wait_media('Metadata', lambda value: 'mpris:artUrl' in value)
        fields = dict(re.findall(r"'([^']+)': <(.*?)>(?=,|})", metadata, re.S))
        track = ast.literal_eval(fields['mpris:trackid'].removeprefix('objectpath '))
        assert track.endswith(current.replace('-', '')), (track, current)
        for field, expected in {'xesam:title':'Track 002', 'xesam:album':'Northern Lights',
                'xesam:artist':['Fixture Artist', 'Guest Artist'], 'xesam:albumArtist':['Album Artist'],
                'xesam:genre':['Ambient', 'Electronic'], 'xesam:comment':['Fixture comment'],
                'xesam:contentCreated':'2026-01-02', 'xesam:lastUsed':'2026-02-03T01:02:03Z',
                'xesam:audioBPM':123, 'xesam:discNumber':1, 'xesam:trackNumber':2,
                'xesam:userRating':.8}.items():
            assert ast.literal_eval(fields[field]) == expected, (field, fields.get(field))
        assert int(fields['mpris:length'].removeprefix('int64 ')) == 40_000_000
        assert int(fields['xesam:useCount']) >= 7
        assert 'This is the local playback fixture' in self.wait_media('Metadata', lambda value: 'xesam:asText' in value)
        from urllib.parse import unquote, urlparse
        cover = urlparse(ast.literal_eval(fields['mpris:artUrl']))
        assert cover.scheme == 'file' and Path(unquote(cover.path)).is_file(), cover
        with (profile / 'mpris-signals.log').open('w') as signals:
            monitor = subprocess.Popen(['gdbus', 'monitor', '--session', '--dest',
                'org.mpris.MediaPlayer2.Feishin', '--object-path', '/org/mpris/MediaPlayer2'],
                env=self.environment, stdout=signals, stderr=subprocess.STDOUT)
            try:
                time.sleep(.3)
                self.mpris_command('Play')
                self.wait_media('PlaybackStatus', lambda value: value == "(<'Playing'>,)")
                self.wait_progress(progress, lambda value: value > .5)
                self.mpris_command('Pause')
                self.wait_media('PlaybackStatus', lambda value: value == "(<'Paused'>,)")
                self.mpris_command('SetPosition', track, 20_000_000)
                self.wait_progress(progress, lambda value: abs(value - 20.) < .5)
                for bad_track, position in [('/stale', 10_000_000), (track, -1), (track, 41_000_000)]:
                    self.mpris_command('SetPosition', bad_track, position)
                    time.sleep(.2)
                    assert abs(self.progress(progress) - 20.) < .5
                self.mpris_command('Seek', -5_000_000)
                self.wait_progress(progress, lambda value: abs(value - 15.) < .5)
                self.mpris_command('Stop')
                self.wait_media('PlaybackStatus', lambda value: value == "(<'Stopped'>,)")
                self.wait_progress(progress, lambda value: value == 0.)
            finally:
                monitor.terminate()
                monitor.wait(timeout=5)
        assert 'Seeked (int64 20000000' in (profile / 'mpris-signals.log').read_text()
        print('Packaged MPRIS: metadata/artwork, volume, shuffle, repeat, transport, '
              'seek signals and stale-position rejection passed', flush=True)
