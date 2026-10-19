#!/usr/bin/env python3
"""Test the native client on disposable real servers, without opening a window.

Docker and FFmpeg are required. Containers have only generated music, fresh data,
a separate bridge network, and randomly allocated localhost ports. They are removed on
success or failure. Vendor images are pinned by digest; reports retain versions.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen
import uuid

ROOT = Path(__file__).resolve().parents[2]
PASSWORD = 'native-fixture-password'


def command(*args):
    result = subprocess.run(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError(f'{args[0]} failed ({result.returncode}):\n{result.stdout}')
    return result.stdout.strip()


def request(base, path, body=None, headers=None):
    data = json.dumps(body).encode() if body is not None else None
    req = Request(base + path, data=data, headers={'Content-Type': 'application/json', **(headers or {})})
    try:
        with urlopen(req, timeout=15) as response:
            value = response.read(1024 * 1024)
    except HTTPError as error:
        detail = error.read(4096).decode('utf-8', errors='replace').replace(PASSWORD, '[fixture password]')
        error.msg = f'{path}: {error.reason}: {detail}'
        raise
    if not value:
        return None
    try:
        return json.loads(value)
    except json.JSONDecodeError:
        return value.decode('utf-8')


def wait(base, path):
    deadline = time.monotonic() + 90
    while True:
        try:
            return request(base, path)
        except (HTTPError, URLError, ConnectionError, TimeoutError):
            if time.monotonic() > deadline:
                raise RuntimeError(f'Server did not start: {path}') from None
            time.sleep(0.5)


def setup_navidrome(base):
    wait(base, '/ping')
    # This is the server's first-user setup endpoint, on a fresh database.
    request(base, '/auth/createAdmin', {'username': 'nativefixture', 'password': PASSWORD})
    request(base, '/rest/startScan.view?' + urlencode({'u': 'nativefixture', 'p': PASSWORD, 'v': '1.16.1', 'c': 'feishin-native-fixture', 'f': 'json'}))
    return 'nativefixture', PASSWORD


def setup_jellyfin(base):
    # Public system info is available while database migrations still return 503
    # for the startup controller. Wait for the controller itself.
    wait(base, '/Startup/Configuration')
    request(base, '/Startup/Configuration', {'ServerName': 'Native fixture', 'UICulture': 'en-US', 'MetadataCountryCode': 'US', 'PreferredMetadataLanguage': 'en'})
    request(base, '/Startup/User')
    request(base, '/Startup/User', {'Name': 'nativefixture', 'Password': PASSWORD})
    request(base, '/Startup/RemoteAccess', {'EnableRemoteAccess': True, 'EnableAutomaticPortMapping': False})
    fetchers = ['MusicBrainz', 'TheAudioDB', 'Fanart', 'Lastfm', 'Last.fm']
    options = {'EnableRealtimeMonitor': False, 'SaveLocalMetadata': False,
               'MetadataOptions': [{'ItemType': kind, 'DisabledMetadataFetchers': fetchers, 'DisabledImageFetchers': fetchers}
                                   for kind in ['MusicArtist', 'MusicAlbum', 'Audio']]}
    request(base, '/Library/VirtualFolders?' + urlencode({'name': 'Native music', 'collectionType': 'music', 'paths': '/music', 'refreshLibrary': 'false'}), {'LibraryOptions': options})
    request(base, '/Startup/Complete', {})
    auth = request(base, '/Users/AuthenticateByName', {'Username': 'nativefixture', 'Pw': PASSWORD, 'Password': PASSWORD},
                   {'Authorization': 'MediaBrowser Client="Feishin fixture", Device="Docker", DeviceId="native-fixture", Version="1.0.0"'})
    headers = {'Authorization': f'MediaBrowser Client="Feishin fixture", Device="Docker", DeviceId="native-fixture", Version="1.0.0", Token="{auth["AccessToken"]}"'}
    deadline = time.monotonic() + 90
    started = False
    while True:
        tasks = request(base, '/ScheduledTasks', headers=headers)
        scan = next(task for task in tasks if task['Key'] == 'RefreshLibrary')
        if started and scan['State'] == 'Idle' and (scan.get('LastExecutionResult') or {}).get('Status') == 'Completed':
            break
        if scan['State'] == 'Idle':
            request(base, '/ScheduledTasks/Running/' + scan['Id'], {}, headers)
            started = True
        if time.monotonic() > deadline:
            raise RuntimeError('Jellyfin fixture scan did not finish')
        time.sleep(0.5)
    return 'nativefixture', PASSWORD


def setup_gonic(base):
    wait(base, '/rest/ping.view?' + urlencode({'u': 'admin', 'p': 'admin', 'v': '1.16.1', 'c': 'feishin-native-fixture', 'f': 'json'}))
    request(base, '/rest/startScan.view?' + urlencode({'u': 'admin', 'p': 'admin', 'v': '1.16.1', 'c': 'feishin-native-fixture', 'f': 'json'}))
    return 'admin', 'admin'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--legacy', action='store_true', help='Test older server compatibility branches instead of current fixtures.')
    args = parser.parse_args()
    subprocess.run(['cargo', 'build', '--locked', '-p', 'feishin-core', '--example', 'live_server'], cwd=ROOT, check=True)
    ffmpeg = os.environ.get('FEISHIN_TEST_FFMPEG') or shutil.which('ffmpeg')
    if not ffmpeg:
        raise RuntimeError('FFmpeg is required')
    name = f'feishin-live-{uuid.uuid4().hex[:10]}'
    network = name + '-network'
    command('docker', 'network', 'create', network)
    containers = []
    try:
        with tempfile.TemporaryDirectory(prefix=name + '-') as tmp:
            tmp = Path(tmp)
            music = tmp / 'music' / 'Native Test Artist' / 'Native Parity Test'
            music.mkdir(parents=True)
            cover = ROOT / 'assets/icons/512x512.png'
            shutil.copyfile(cover, music / 'folder.png')
            for index in range(1, 4):
                path = music / f'{index:02}-native.flac'
                subprocess.run([ffmpeg, '-nostdin', '-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i', f'sine=frequency={300 + index * 100}:sample_rate=48000:duration=2', '-i', str(cover), '-map', '0:a', '-map', '1:v', '-ac', '2', '-c:a', 'flac', '-c:v', 'png', '-disposition:v', 'attached_pic', '-metadata', f'title=Native Track {index}', '-metadata', 'artist=Native Test Artist', '-metadata', 'album_artist=Native Test Artist', '-metadata', 'album=Native Parity Test', '-metadata', f'track={index}', '-metadata', 'disc=1', '-metadata', 'genre=Ambient', '-metadata', 'subtitle=Fixture movement', '-metadata', 'date=2026', str(path)], check=True)
                path.with_suffix('.lrc').write_text('[00:00.00]Native fixture verse\n[00:01.00]Native fixture chorus\n')
            fixtures = [
                ('navidrome', 'deluan/navidrome@sha256:38dc2727bfcfd5ede290f8ada114fc90368146f265ae4701ddddbcbe2a44ee52', 4533, {'ND_ENABLEINSIGHTS': 'false', 'ND_SCANSCHEDULE': '0', 'ND_LOGLEVEL': 'warn'}, setup_navidrome),
                ('jellyfin', 'jellyfin/jellyfin@sha256:357724bf0ae27a672c7cbaa899db2d9abeb13dbd8657ccce750258a4c059d037', 8096, {'JELLYFIN_playlists__allowDuplicates': 'true'}, setup_jellyfin),
                ('subsonic', 'sentriz/gonic@sha256:516fd9645614ba3a596d86174216c3e944808b9ec970c581678713be4c8b1d49', 80, {'GONIC_SCAN_AT_START_ENABLED': 'true', 'GONIC_CACHE_PATH': '/data/cache', 'GONIC_PODCAST_PATH': '/data/podcasts', 'GONIC_PLAYLISTS_PATH': '/data/playlists'}, setup_gonic),
            ]
            if args.legacy:
                # Navidrome 0.54.5, Jellyfin 10.9.11, Gonic 0.16.4.
                images = [
                    'deluan/navidrome@sha256:2a4deb64be3d42f245947b4330988bb8b48b99271f6afdaf68384ed3c3de9d51',
                    'jellyfin/jellyfin@sha256:fc1b51f4be3fba725e42dae2022d9c6a5b069acce01bef04d32fdee025dc511e',
                    'sentriz/gonic@sha256:22e2c9de7a95606f196e1828e6ba0224aa29245cba42cb19eeb4322f6dae4c7b',
                ]
                fixtures = [(kind, image, port, env, setup)
                            for (kind, _, port, env, setup), image in zip(fixtures, images, strict=True)]
            for kind, image, port, env, setup in fixtures:
                container = name + '-' + kind
                containers.append(container)
                data = tmp / kind
                data.mkdir(mode=0o777)
                data.chmod(0o777)
                for folder in ['cache', 'podcasts', 'playlists']:
                    (data / folder).mkdir(mode=0o777)
                    (data / folder).chmod(0o777)
                mounts = ['-v', f'{tmp / "music"}:/music:ro']
                mounts += ['-v', f'{data}:/config', '-v', f'{data / "cache"}:/cache'] if kind == 'jellyfin' else ['-v', f'{data}:/data']
                env_args = [arg for key, value in env.items() for arg in ['-e', f'{key}={value}']]
                command('docker', 'run', '-d', '--name', container, '--network', network, '--cpus', '1', '--memory', '1g', '-p', f'127.0.0.1::{port}', *mounts, *env_args, image)
                published = json.loads(command('docker', 'inspect', container))[0]['NetworkSettings']['Ports'][f'{port}/tcp'][0]['HostPort']
                base = f'http://127.0.0.1:{published}'
                print(f'Testing {kind} on disposable localhost server', flush=True)
                try:
                    username, password = setup(base)
                    probe = ROOT / 'target/debug/examples/live_server'
                    result = json.loads(command(str(probe), kind, base, username, password))
                    if kind == 'jellyfin':
                        result['version'] = request(base, '/System/Info/Public')['Version']
                    result['image'] = json.loads(command('docker', 'image', 'inspect', image))[0]['RepoDigests'][0]
                    print(json.dumps(result, sort_keys=True), flush=True)
                except Exception:
                    print(command('docker', 'logs', '--tail', '30', container), flush=True)
                    raise
                finally:
                    command('docker', 'rm', '-f', container)
                    containers.remove(container)
    finally:
        for container in containers:
            subprocess.run(['docker', 'rm', '-f', container], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(['docker', 'network', 'rm', network], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


if __name__ == '__main__':
    main()
