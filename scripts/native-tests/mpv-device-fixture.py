#!/usr/bin/env python3
import json, socket, sys
args = sys.argv[1:]
path = next(a.split('=', 1)[1] for a in args if a.startswith('--input-ipc-server='))
log = next(a.split('=', 1)[1] for a in args if a.startswith('--log-file='))
listener = socket.socket(socket.AF_UNIX)
listener.bind(path)
listener.listen(1)
connection, _ = listener.accept()
with connection, connection.makefile('r') as reader, open(log, 'a') as output:
    for line in reader:
        request = json.loads(line)
        output.write(line); output.flush()
        if request['command'] == ['get_property', 'audio-device-list']:
            response = {'request_id': request['request_id'], 'error': 'property unavailable' if '--fail' in args else 'success', 'data': [
                {'name': 'auto', 'description': 'Automatic'},
                {'name': 'coreaudio/42', 'description': 'Studio DAC'},
                {'name': 'coreaudio/42', 'description': 'Duplicate'},
                {'name': 'null', 'description': ''}, {'description': 'Invalid'}]}
            connection.sendall(b'{"event":"property-change","name":"pause","data":true}\n')
            connection.sendall((json.dumps(response) + '\n').encode())
        else:
            connection.sendall((json.dumps({'request_id': request['request_id'], 'error': 'success'}) + '\n').encode())
