#!/usr/bin/env python3
"""Pin native TagLib for metadata formats unsupported by Lofty."""
import hashlib
import io
import pathlib
import tarfile
import urllib.request

VERSION = '2.3.2'
SHA256 = '3ca2d8afaa7f1cf7f6ed10e511ebc368bfacd6dcaa3dbfa690b89e502e8963dc'
ROOT = pathlib.Path(__file__).resolve().parent.parent
DEST = ROOT / 'crates/vendor/taglib'
URL = f'https://taglib.org/releases/taglib-{VERSION}.tar.gz'

def main():
    with urllib.request.urlopen(URL, timeout=60) as response:
        data = response.read()
    assert hashlib.sha256(data).hexdigest() == SHA256, 'TagLib source checksum mismatch'
    archive = tarfile.open(fileobj=io.BytesIO(data), mode='r:gz')
    for entry in archive:
        if not entry.isfile():
            continue
        relative = pathlib.PurePosixPath(entry.name).parts[1:]
        assert relative and '..' not in relative
        path = DEST.joinpath(*relative)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(archive.extractfile(entry).read())
    (DEST / 'FEISHIN-SOURCE.md').write_text(f'# Native TagLib\n\nUnmodified TagLib {VERSION} source from {URL}.\nSHA256: {SHA256}.\n\nThe native metadata fallback statically links this source for formats not handled\nby Lofty. Select the LGPL-2.1-or-later license from TagLib\'s dual-license terms;\nsee COPYING.LGPL. Build instructions and the complete source allow relinking.\n')
    print(f'Bundled TagLib {VERSION}')

if __name__ == '__main__':
    main()
