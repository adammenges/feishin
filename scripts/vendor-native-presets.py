#!/usr/bin/env python3
"""Reproduce the native archive from the MilkDrop sources used by Butterchurn."""
import io
import hashlib
import base64
import pathlib
import re
import tarfile
import urllib.request
import zipfile

REVISION = "c10e2616762f262de898e5d4c1f162a3e01340c1"
URL = f"https://codeload.github.com/jberg/butterchurn-presets/tar.gz/{REVISION}"
BUTTERCHURN_VERSION = "3.0.0-beta.5"
BUTTERCHURN_SHA256 = "4bf374256652c08ef077727e346dfdb6381e598099bbadc3da4a573b7b037c39"
DEST = pathlib.Path(__file__).resolve().parent.parent / "crates/feishin-core/assets"

def normalize_equations(text):
    # The legacy MilkDrop evaluator ignores whitespace inside identifiers.
    # projectM-eval tokenizes it; concatenate equation rows after removing comments.
    pattern = re.compile(r"^((?:per_frame_init_|per_frame_|per_pixel_|(?:wave|shape)_\d+_(?:init|per_frame|per_point)_?))\d+=(.*)$")
    groups = {}
    lines = text.splitlines()
    for line in lines:
        match = pattern.match(line)
        if match:
            prefix, code = match.groups()
            groups.setdefault(prefix, []).append(code.split("//", 1)[0])
    output = []
    for line in lines:
        match = pattern.match(line)
        if not match:
            output.append(line)
        elif match.group(1) in groups:
            prefix = match.group(1)
            code = "".join(groups.pop(prefix))
            code = re.sub(r"/\*.*?\*/", "", code, flags=re.S)
            output.append(prefix + "1=" + re.sub(r"\s+", "", code))
    return "\n".join(output) + "\n"

def main():
    with urllib.request.urlopen(URL, timeout=60) as response:
        archive = tarfile.open(fileobj=io.BytesIO(response.read()), mode="r:gz")
    files = {entry.name.split("/", 1)[1]: archive.extractfile(entry).read()
             for entry in archive if entry.isfile()}
    names = []
    for pack in ("base.js", "extra.js", "md1.js"):
        for name in re.findall(r"require\(['\"]\./presets/converted/(.*?)\.json['\"]\)", files[pack].decode()):
            if name not in names and name != "Flexi + Martin - astral projection":
                names.append(name)
    originals = {path.rsplit("/", 1)[1][:-5]: data for path, data in files.items() if path.endswith(".milk")}
    assert all(name in originals for name in names), "Missing original MilkDrop presets"
    with zipfile.ZipFile(DEST / "milkdrop-presets.zip", "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as output:
        for name, data in [(name + ".milk", originals[name]) for name in names] + [("LICENSE", files["LICENSE"])]:
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            # The originals occasionally contain Windows-1252 comments.
            try:
                text = data.decode("utf-8")
            except UnicodeDecodeError:
                text = data.decode("cp1252")
            if name.endswith(".milk"):
                text = normalize_equations(text)
            output.writestr(info, text.encode("utf-8"), compresslevel=9)
    (DEST / "milkdrop-LICENSE.txt").write_bytes(files["LICENSE"])
    images = re.findall(r'  (\w+): \{\s*data:\s*"data:image/(\w+);base64,([^"]+)"', files["imageData.js"].decode())
    assert len(images) == 6
    url = f"https://registry.npmjs.org/butterchurn/-/butterchurn-{BUTTERCHURN_VERSION}.tgz"
    with urllib.request.urlopen(url, timeout=60) as response:
        data = response.read()
    assert hashlib.sha256(data).hexdigest() == BUTTERCHURN_SHA256, "Butterchurn source checksum mismatch"
    source = tarfile.open(fileobj=io.BytesIO(data), mode="r:gz")
    js = source.extractfile("package/dist/butterchurn.js").read().decode()
    cloud = re.search(r'this\.clouds2Image\.src\s*=\s*"data:image/(\w+);base64,([^"]+)"', js)
    assert cloud, "Missing original fallback texture"
    fmt, image = cloud.groups()
    images.append(("clouds2", fmt, image))
    (DEST / "butterchurn-LICENSE.txt").write_bytes(source.extractfile("package/LICENSE").read())
    with zipfile.ZipFile(DEST / "milkdrop-textures.zip", "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as output:
        for name, fmt, data in images:
            info = zipfile.ZipInfo(name + "." + ("jpg" if fmt == "jpeg" else fmt), date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            output.writestr(info, base64.b64decode(data), compresslevel=9)
    print(f"Bundled {len(names)} presets from {REVISION}")

if __name__ == "__main__":
    main()
