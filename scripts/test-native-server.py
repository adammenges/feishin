#!/usr/bin/env python3
"""Disposable localhost Navidrome/Subsonic fixture for native UI and audio checks.

Run: python3 scripts/test-native-server.py
Sign in: Navidrome, http://127.0.0.1:45339, test / test.
No external services or user data are used. All edits live in memory.
"""
import io
import json
import math
import struct
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

ROOT = Path(__file__).resolve().parents[1]
ALBUMS = [dict(id=f"album{i}", name=name, artist="Fixture Artist", year=2026,
               albumArtist="Fixture Artist", albumArtistId="artist0",
               minYear=2026, maxYear=2026, releaseDate="2026-01-02",
               genres=[dict(id="genre0", name="Ambient")],
               duration=1280, songCount=32, coverArt=f"album{i}") for i, name in enumerate(
                   ["Northern Lights", "Quiet Hours", "Open Skies", "Night Signals"])]
SONGS = [dict(id=f"song{i}", title=f"Track {i + 1:03}", artist="Fixture Artist",
              artistId="artist0", albumId=f"album{i // 32}",
              album=ALBUMS[i // 32]["name"], duration=40, trackNumber=i % 32 + 1,
              discNumber=1, genre="Ambient", year=2026, coverArt=f"album{i // 32}",
              starred=False, rating=0) for i in range(128)]
PLAYLISTS = [dict(id="playlist0", name="Test playlist", public=False, comment="Disposable fixture",
                  songCount=3, duration=120, ownerId="user0", ownerName="test", rules=None)]
ARTISTS = [dict(id="artist0", name="Fixture Artist", coverArt="album0")]
TRACKS = {"playlist0": [dict(song, mediaFileId=song["id"], id=str(i + 1)) for i, song in enumerate(SONGS[:3])]}

buf = io.BytesIO()
with wave.open(buf, "wb") as wav:
    wav.setnchannels(1)
    wav.setsampwidth(2)
    wav.setframerate(8000)
    wav.writeframes(b"".join(struct.pack("<h", int(2000 * math.sin(2 * math.pi * 220 * i / 8000))) for i in range(320000)))
AUDIO = buf.getvalue()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def response(self, data, status=200, content_type="application/json", total=None, headers=None):
        body = data if isinstance(data, bytes) else json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        if total is not None:
            self.send_header("X-Total-Count", str(total))
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def body(self):
        return json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")

    def do_GET(self):
        parsed = urlparse(self.path)
        path, query = parsed.path, parse_qs(parsed.query)
        if path.startswith("/rest/"):
            endpoint = path.split("/")[-1].removesuffix(".view")
            envelope = {"status": "ok", "version": "1.16.1"}
            if endpoint == "stream":
                data, headers, status = AUDIO, {"Accept-Ranges": "bytes"}, 200
                if self.headers.get("Range"):
                    start = int(self.headers["Range"].split("=")[1].split("-")[0])
                    data, status = AUDIO[start:], 206
                    headers["Content-Range"] = f"bytes {start}-{len(AUDIO)-1}/{len(AUDIO)}"
                self.response(data, status, "audio/wav", headers=headers)
                return
            if endpoint == "getCoverArt":
                self.response((ROOT / "assets/icons/icon.png").read_bytes(), content_type="image/png")
                return
            if endpoint == "getLyricsBySongId":
                envelope["lyricsList"] = {"structuredLyrics": [{"synced": True, "line": [
                    {"start": i * 5000, "value": text} for i, text in enumerate([
                        "This is the local playback fixture", "Lyrics follow the song", "Seek to another line", "Pause and resume playback"]) ]}]}
            elif endpoint == "getUser":
                envelope["user"] = {"username": "test", "adminRole": True}
            elif endpoint == "getGenres":
                envelope["genres"] = {"genre": [{"value": "Ambient", "songCount": 128, "albumCount": 4}]}
            elif endpoint == "getRandomSongs":
                envelope["randomSongs"] = {"song": SONGS[:int(query.get("size", [25])[0])]}
            elif endpoint == "getInternetRadioStations":
                envelope["internetRadioStations"] = {"internetRadioStation": []}
            elif endpoint == "getPlayQueue":
                envelope["playQueue"] = {"entry": SONGS[:3], "current": "song1", "position": 10000}
            elif endpoint == "star" or endpoint == "unstar":
                ids = query.get("id", [])
                for song in SONGS:
                    if song["id"] in ids:
                        song["starred"] = endpoint == "star"
            elif endpoint == "setRating":
                for song in SONGS:
                    if song["id"] == query.get("id", [""])[0]:
                        song["rating"] = int(query["rating"][0])
            self.response({"subsonic-response": envelope})
            return
        if path.startswith("/api/"):
            if self.headers.get("x-nd-authorization") != "Bearer fixture-token":
                self.response({"error": "Unauthorized"}, 401)
                return
            catalog = {"/api/album": ALBUMS, "/api/song": SONGS,
                       "/api/artist": ARTISTS,
                       "/api/genre": [dict(id="genre0", name="Ambient")],
                       "/api/tag": [dict(id="genre0", tagName="genre", tagValue="Ambient",
                                         albumCount=4, songCount=128)],
                       "/api/playlist": PLAYLISTS}
            if path.startswith("/api/playlist/") and path.endswith("/tracks"):
                values = TRACKS.get(path.split("/")[3], [])
            else:
                collection, _, item_id = path.rpartition("/")
                if collection in catalog:
                    item = next((item for item in catalog[collection] if item["id"] == item_id), None)
                    self.response(item or {"error": "Not found"}, 200 if item else 404)
                    return
                values = catalog.get(path, [])
            values = list(values)
            for key, field in [("album_id", "albumId"), ("title", "title"), ("name", "name"),
                               ("tag_name", "tagName"), ("tag_value", "tagValue")]:
                if key in query:
                    values = [v for v in values if query[key][0].lower() in str(v.get(field, "")).lower()]
            if query.get("starred") == ["true"]:
                values = [v for v in values if v.get("starred")]
            start, end = int(query.get("_start", [0])[0]), int(query.get("_end", [0])[0])
            # Navidrome's REST controller treats a nonpositive range as unlimited.
            # Its repository clamps negative offsets and only adds a positive LIMIT.
            limit, start = max(0, end - start), max(0, start)
            self.response(values[start:start + limit if limit else None], total=len(values))
            return
        self.response({"error": "Unknown fixture route"}, 404)

    def do_POST(self):
        path = urlparse(self.path).path
        data = self.body()
        if path == "/auth/login":
            if data.get("username") != "test" or data.get("password") != "test":
                self.response({"error": "Use test / test"}, 401)
                return
            self.response(dict(token="fixture-token", id="user0", username="test", isAdmin=True,
                               subsonicSalt="fixture-salt", subsonicToken="fixture-hash"))
        elif path == "/api/playlist":
            playlist = dict(data, id=f"playlist{len(PLAYLISTS)}")
            PLAYLISTS.append(playlist)
            TRACKS[playlist["id"]] = []
            self.response(playlist)
        elif path.endswith("/tracks"):
            tracks = TRACKS.setdefault(path.split("/")[3], [])
            for id_ in data.get("ids", []):
                song = next(s for s in SONGS if s["id"] == id_)
                tracks.append(dict(song, mediaFileId=id_, id=str(len(tracks) + 1)))
            self.response({})
        else:
            self.response({})

    def do_PUT(self):
        id_ = urlparse(self.path).path.split("/")[-1]
        data = self.body()
        for playlist in PLAYLISTS:
            if playlist["id"] == id_:
                playlist.update(data)
        self.response({})

    def do_DELETE(self):
        parsed = urlparse(self.path)
        if parsed.path.endswith("/tracks"):
            ids = parse_qs(parsed.query).get("id", [])
            id_ = parsed.path.split("/")[3]
            TRACKS[id_] = [track for track in TRACKS.get(id_, []) if track["id"] not in ids]
        self.response({})


if __name__ == "__main__":
    print("Native fixture: http://127.0.0.1:45339 (test / test)", flush=True)
    ThreadingHTTPServer(("127.0.0.1", 45339), Handler).serve_forever()
