#!/usr/bin/env python3
"""Exercise real launch/restart CLI paths without opening a native window."""
import json
import os
import pathlib
import socket
import subprocess
import sys
import tempfile
import threading
import time
import uuid

binary = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else 'target/debug/feishin').resolve()
assert binary.is_file(), f'Native binary is missing: {binary}'
with tempfile.TemporaryDirectory(prefix='feishin-lifecycle-') as directory:
    root = pathlib.Path(directory) / "profile with spaces ' $&"
    root.mkdir()
    state = root / 'state.json'
    state.write_text('{"fixture":"preserve without loading"}')
    token = uuid.uuid4()
    with (root / 'instance.lock').open('w+b') as lock, socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as listener:
        lock.write(b'x')
        lock.flush()
        lock.seek(0)
        if os.name == 'nt':
            import msvcrt
            msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        listener.bind(('127.0.0.1', 0))
        listener.settimeout(0.1)
        endpoint = root / 'instance.json'
        endpoint.write_text(json.dumps({'address': f'127.0.0.1:{listener.getsockname()[1]}', 'token': str(token)}))
        if os.name != 'nt':
            endpoint.chmod(0o600)
        activations = []
        stop = threading.Event()

        def acknowledge():
            while not stop.is_set():
                try:
                    message, sender = listener.recvfrom(64)
                except socket.timeout:
                    continue
                if message == token.bytes:
                    activations.append(time.monotonic())
                    listener.sendto(message, sender)

        worker = threading.Thread(target=acknowledge)
        worker.start()
        environment = dict(os.environ, FEISHIN_DATA_DIR=str(root))
        parent = None
        restarted = None
        try:
            launched = subprocess.run([str(binary)], env=environment, capture_output=True, text=True, timeout=15)
            assert launched.returncode == 0, launched.stderr
            assert len(activations) == 1
            assert state.read_text() == '{"fixture":"preserve without loading"}'
            assert not (root / 'logs').exists(), 'Second launch initialized the application'
            for arguments in [['--restart-after'], ['--restart-after', 'invalid'], ['--restart-after', '0']]:
                failed = subprocess.run([str(binary), *arguments], env=environment, capture_output=True, text=True, timeout=5)
                assert failed.returncode != 0, arguments
            parent = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'])
            restarted = subprocess.Popen([str(binary), '--restart-after', str(parent.pid)], env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            time.sleep(0.2)
            assert restarted.poll() is None, 'Restart did not wait for its parent'
            assert len(activations) == 1
            parent.terminate()
            parent.wait(timeout=5)
            _, errors = restarted.communicate(timeout=15)
            assert restarted.returncode == 0, errors
            assert len(activations) == 2
            assert state.read_text() == '{"fixture":"preserve without loading"}'
            assert not (root / 'logs').exists()
        finally:
            for child in [restarted, parent]:
                if child is not None and child.poll() is None:
                    child.kill()
                    child.wait(timeout=5)
            stop.set()
            worker.join(timeout=2)
            assert not worker.is_alive()
print('Native lifecycle CLI: second-launch activation, deferred restart, invalid arguments and profile preservation passed')
