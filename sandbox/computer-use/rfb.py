"""A small RFB (VNC) client for the macOS computer-use suite.

The macOS suite drives a Tart VM through Virtualization.framework's own VNC
server (`tart run --vnc-experimental`): screenshots come from the VM's
framebuffer and input arrives in the guest as hardware events, so nothing in
the guest needs Screen Recording or Accessibility permission
(docs/DECISIONS.md, 2026-11-11). vncdotool hangs against that server
(disconnect and incremental refresh), so this is a minimal synchronous
client: protocol 3.8, VNC or no authentication, 32-bit true colour, Raw
encoding plus the DesktopSize pseudo-encoding.

What the server does with keys, found in the 2026-11-11 spike: it maps key
codes, not keysyms. A capital letter or a shifted symbol needs Shift held
explicitly, the keysym Alt_L arrives as Command, Meta_L as Option, and
Super_L is ignored. `KeyMapper` turns Mac key names and text into those
events.

Only the standard library is needed, except `cryptography` for the DES step
of VNC authentication.
"""

from __future__ import annotations

import contextlib
import socket
import struct
import threading
import time
from dataclasses import dataclass
from urllib.parse import urlparse

# X11 keysyms the macOS guest understands (VZ maps them to key codes).
KEYSYMS: dict[str, int] = {
    "return": 0xFF0D,
    "enter": 0xFF0D,
    "tab": 0xFF09,
    "escape": 0xFF1B,
    "esc": 0xFF1B,
    "backspace": 0xFF08,
    "delete": 0xFF08,  # the Mac "delete" key is backspace
    "forwarddelete": 0xFFFF,
    "home": 0xFF50,
    "end": 0xFF57,
    "pageup": 0xFF55,
    "page_up": 0xFF55,
    "pagedown": 0xFF56,
    "page_down": 0xFF56,
    "left": 0xFF51,
    "up": 0xFF52,
    "right": 0xFF53,
    "down": 0xFF54,
    "space": 0x20,
    "shift": 0xFFE1,
    "ctrl": 0xFFE3,
    "control": 0xFFE3,
    "cmd": 0xFFE9,  # Alt_L arrives as Command
    "command": 0xFFE9,
    "super": 0xFFE9,
    "meta": 0xFFE9,
    "win": 0xFFE9,
    "option": 0xFFE7,  # Meta_L arrives as Option
    "opt": 0xFFE7,
    "alt": 0xFFE7,
    "fn": 0xFFED,
}
for _i in range(1, 13):
    KEYSYMS[f"f{_i}"] = 0xFFBE + _i - 1
MODIFIERS = {
    "shift",
    "ctrl",
    "control",
    "cmd",
    "command",
    "super",
    "meta",
    "win",
    "option",
    "opt",
    "alt",
    "fn",
}
# US layout: shifted symbol -> the unshifted key that produces it.
SHIFTED = dict(zip('!@#$%^&*()_+{}|:"<>?~', "1234567890-=[]\\;',./`", strict=True))


class RFBError(RuntimeError):
    pass


def _des_key(password: str) -> bytes:
    """VNC authentication reverses the bit order of each key byte."""
    raw = password.encode("latin-1")[:8].ljust(8, b"\0")
    return bytes(int(f"{b:08b}"[::-1], 2) for b in raw)


def vnc_response(password: str, challenge: bytes) -> bytes:
    # Imported here so the rest of the module needs only the standard library.
    try:  # cryptography >= 43 keeps DES only under "decrepit"
        from cryptography.hazmat.decrepit.ciphers.algorithms import TripleDES  # noqa: PLC0415
    except ImportError:  # pragma: no cover - older cryptography
        from cryptography.hazmat.primitives.ciphers.algorithms import (  # type: ignore[no-redef]  # noqa: PLC0415
            TripleDES,
        )
    from cryptography.hazmat.primitives.ciphers import Cipher, modes  # noqa: PLC0415

    key = _des_key(password)
    enc = Cipher(TripleDES(key * 3), modes.ECB()).encryptor()
    return enc.update(challenge) + enc.finalize()


@dataclass
class KeyEvent:
    keysym: int
    down: bool


class KeyMapper:
    """Mac key names and plain text -> the key events the VZ server needs."""

    @staticmethod
    def keysym(name: str) -> int:
        low = name.lower()
        if low in KEYSYMS:
            return KEYSYMS[low]
        if len(name) == 1 and 0x20 <= ord(name) < 0x7F:
            return ord(name.lower())
        raise ValueError(f"unknown key {name!r}")

    def chord(self, chord: str) -> list[KeyEvent]:
        """'cmd+shift+4' -> modifiers down, key down/up, modifiers up."""
        if len(chord) == 1:
            parts = [chord]
        elif chord.endswith("++"):  # cmd++ is Command with the plus key
            parts = [p for p in chord[:-2].split("+") if p] + ["+"]
        else:
            parts = [p for p in chord.split("+") if p]
        if not parts:
            raise ValueError("empty key")
        *mods, key = parts
        bad = [m for m in mods if m.lower() not in MODIFIERS]
        if bad:
            raise ValueError(f"not a modifier: {', '.join(bad)}")
        mod_syms = [self.keysym(m) for m in mods]
        if len(key) == 1 and (key.isupper() or key in SHIFTED):
            mod_syms.append(KEYSYMS["shift"])
            key = key.lower() if key.isupper() else SHIFTED[key]
        sym = self.keysym(key)
        events = [KeyEvent(s, True) for s in mod_syms]
        events += [KeyEvent(sym, True), KeyEvent(sym, False)]
        events += [KeyEvent(s, False) for s in reversed(mod_syms)]
        return events

    def text(self, text: str) -> list[KeyEvent]:
        events: list[KeyEvent] = []
        for ch in text:
            if ch == "\n":
                events += self.chord("return")
            elif ch == "\t":
                events += self.chord("tab")
            elif ch == " ":
                events += self.chord("space")
            elif 0x20 < ord(ch) < 0x7F:
                events += self.chord(ch)
            else:
                raise ValueError(
                    f"cannot type {ch!r}: the VM's VNC server takes US-keyboard ASCII only"
                )
        return events


class RFBClient:
    """A VNC viewer over one persistent connection.

    The VZ server answers a non-incremental FramebufferUpdateRequest only
    once per connection, and its VNC server has crashed the whole VM when
    clients connect and disconnect quickly (an assertion in
    _VZVNCServer _setupVirtualMachineAccessor, 2026-11-11). So this client
    connects once, asks for one full frame, and then a reader thread keeps
    an incremental request outstanding, patching a local framebuffer as
    damage arrives. `capture()` returns that framebuffer once updates have
    gone quiet. Input events share the socket under a write lock.
    """

    def __init__(self, url: str, *, timeout: float = 30.0, key_gap: float = 0.03):
        u = urlparse(url)
        if u.scheme != "vnc" or not u.port:
            raise RFBError(f"not a vnc:// URL with a port: {url!r}")
        self.host, self.port, self.password = u.hostname or "127.0.0.1", u.port, u.password or ""
        self.timeout, self.key_gap = timeout, key_gap
        self.lock = threading.RLock()  # connection lifecycle and input sequences
        self.wlock = threading.Lock()  # socket writes
        self.fblock = threading.Lock()
        self.sock: socket.socket | None = None
        self.width = self.height = 0
        self.fb = bytearray()
        self.buttons = 0
        self.pointer = (0, 0)
        self.mapper = KeyMapper()
        self.updates = 0
        self.last_update = 0.0
        self.first_frame = threading.Event()
        self.reader: threading.Thread | None = None
        self.reader_error: BaseException | None = None
        self._stop = threading.Event()

    # -- connection -------------------------------------------------------
    def _recv(self, n: int) -> bytes:
        assert self.sock is not None
        buf = bytearray()
        while len(buf) < n:
            chunk = self.sock.recv(min(1 << 20, n - len(buf)))
            if not chunk:
                raise RFBError("server closed the connection")
            buf += chunk
        return bytes(buf)

    def _send(self, data: bytes) -> None:
        assert self.sock is not None
        with self.wlock:
            self.sock.sendall(data)

    def connect(self) -> None:
        with self.lock:
            self.close()
            sock = socket.create_connection((self.host, self.port), timeout=self.timeout)
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            self.sock = sock
            try:
                self._handshake()
            except BaseException:
                self.close()
                raise
            sock.settimeout(None)  # the reader blocks until the server sends
            self._stop.clear()
            self.first_frame.clear()
            self.reader_error = None
            self.reader = threading.Thread(target=self._read_loop, name="rfb-reader", daemon=True)
            self.reader.start()
            self._request(incremental=False)
            if not self.first_frame.wait(self.timeout):
                self.close()
                raise RFBError("no first frame from the server")

    def _handshake(self) -> None:
        version = self._recv(12)
        if not version.startswith(b"RFB 003."):
            raise RFBError(f"not an RFB server: {version!r}")
        self._send(b"RFB 003.008\n")
        (count,) = self._recv(1)
        if count == 0:
            (n,) = struct.unpack(">I", self._recv(4))
            raise RFBError(f"server refused: {self._recv(n).decode(errors='replace')}")
        types = list(self._recv(count))
        if 2 in types:
            self._send(b"\x02")
            self._send(vnc_response(self.password, self._recv(16)))
        elif 1 in types:
            self._send(b"\x01")
        else:
            raise RFBError(f"no supported security type in {types}")
        (result,) = struct.unpack(">I", self._recv(4))
        if result != 0:
            raise RFBError("VNC authentication failed")
        self._send(b"\x01")  # ClientInit, shared
        self.width, self.height = struct.unpack(">HH", self._recv(4))
        self._recv(16)  # server pixel format, replaced below
        (name_len,) = struct.unpack(">I", self._recv(4))
        self._recv(name_len)
        # 32 bpp, depth 24, little endian, true colour: BGRX in memory.
        pf = struct.pack(">BBBBHHHBBB3x", 32, 24, 0, 1, 255, 255, 255, 16, 8, 0)
        self._send(b"\x00\x00\x00\x00" + pf)
        encodings = [0, -223]  # Raw, DesktopSize
        self._send(
            struct.pack(">BxH", 2, len(encodings))
            + b"".join(struct.pack(">i", e) for e in encodings)
        )
        with self.fblock:
            self.fb = bytearray(self.width * self.height * 4)

    def _request(self, incremental: bool) -> None:
        self._send(struct.pack(">BBHHHH", 3, int(incremental), 0, 0, self.width, self.height))

    def close(self) -> None:
        with self.lock:
            self._stop.set()
            if self.sock is not None:
                with contextlib.suppress(OSError):
                    self.sock.shutdown(socket.SHUT_RDWR)
                with contextlib.suppress(OSError):
                    self.sock.close()
                self.sock = None
            if self.reader is not None and self.reader is not threading.current_thread():
                self.reader.join(timeout=5)
            self.reader = None

    def alive(self) -> bool:
        return self.sock is not None and self.reader is not None and self.reader.is_alive()

    def _ensure(self) -> None:
        if not self.alive():
            self.connect()

    def _retry(self, fn):  # type: ignore[no-untyped-def]
        """Run fn; on a broken connection reconnect once and run it again."""
        with self.lock:
            try:
                self._ensure()
                return fn()
            except (OSError, RFBError):
                self.connect()
                return fn()

    # -- screen -----------------------------------------------------------
    def _read_loop(self) -> None:  # noqa: PLR0912 - one branch per RFB message type
        try:
            while not self._stop.is_set():
                (kind,) = self._recv(1)
                if kind == 0:  # FramebufferUpdate
                    (nrects,) = struct.unpack(">xH", self._recv(3))
                    resized = False
                    for _ in range(nrects):
                        x, y, w, h, enc = struct.unpack(">HHHHi", self._recv(12))
                        if enc == 0:
                            data = self._recv(w * h * 4)
                            row = w * 4
                            with self.fblock:
                                for r in range(h):
                                    off = ((y + r) * self.width + x) * 4
                                    self.fb[off : off + row] = data[r * row : (r + 1) * row]
                        elif enc == -223:  # DesktopSize
                            with self.fblock:
                                self.width, self.height = w, h
                                self.fb = bytearray(w * h * 4)
                            resized = True
                        else:
                            raise RFBError(f"unexpected encoding {enc}")
                    self.updates += 1
                    self.last_update = time.monotonic()
                    self.first_frame.set()
                    self._request(incremental=not resized)
                elif kind == 1:  # SetColourMapEntries
                    _first, n = struct.unpack(">xHH", self._recv(5))
                    self._recv(n * 6)
                elif kind == 2:  # Bell
                    continue
                elif kind == 3:  # ServerCutText
                    (n,) = struct.unpack(">3xI", self._recv(7))
                    self._recv(n)
                else:
                    raise RFBError(f"unknown server message {kind}")
        except BaseException as exc:  # reported through alive()
            if not self._stop.is_set():
                self.reader_error = exc

    def capture(self, quiet: float = 0.15, max_wait: float = 2.0) -> tuple[int, int, bytes]:
        """The current screen, once no update has arrived for `quiet` seconds
        (or after `max_wait`): (width, height, BGRX bytes)."""

        def go() -> tuple[int, int, bytes]:
            t0 = time.monotonic()
            while time.monotonic() - t0 < max_wait:
                if not self.alive():
                    raise RFBError(f"connection lost: {self.reader_error}")
                if time.monotonic() - self.last_update >= quiet:
                    break
                time.sleep(0.02)
            with self.fblock:
                return self.width, self.height, bytes(self.fb)

        return self._retry(go)

    # -- input ------------------------------------------------------------
    def _pointer(self, x: int, y: int, buttons: int) -> None:
        x = max(0, min(self.width - 1, int(x)))
        y = max(0, min(self.height - 1, int(y)))
        self._send(struct.pack(">BBHH", 5, buttons, x, y))
        self.pointer, self.buttons = (x, y), buttons

    def move(self, x: int, y: int) -> None:
        self._retry(lambda: self._pointer(x, y, self.buttons))

    def click(self, x: int, y: int, button: int = 1, count: int = 1, gap: float = 0.08) -> None:
        mask = 1 << (button - 1)

        def go() -> None:
            self._pointer(x, y, 0)
            time.sleep(0.05)
            for i in range(count):
                self._pointer(x, y, mask)
                time.sleep(0.03)
                self._pointer(x, y, 0)
                if i + 1 < count:
                    time.sleep(gap)

        self._retry(go)

    def button(self, x: int, y: int, button: int, down: bool) -> None:
        mask = 1 << (button - 1)
        self._retry(
            lambda: self._pointer(x, y, (self.buttons | mask) if down else (self.buttons & ~mask))
        )

    def scroll(self, x: int, y: int, direction: str, amount: int) -> None:
        bit = {"up": 4, "down": 5, "left": 6, "right": 7}[direction]

        def go() -> None:
            self._pointer(x, y, 0)
            for _ in range(amount):
                self._pointer(x, y, 1 << (bit - 1))
                self._pointer(x, y, 0)
                time.sleep(0.03)

        self._retry(go)

    def send_keys(self, events: list[KeyEvent]) -> None:
        def go() -> None:
            for ev in events:
                self._send(struct.pack(">BBxxI", 4, 1 if ev.down else 0, ev.keysym))
                time.sleep(self.key_gap)

        self._retry(go)

    def key(self, chord: str) -> None:
        self.send_keys(self.mapper.chord(chord))

    def type_text(self, text: str) -> None:
        self.send_keys(self.mapper.text(text))


def png_bytes(width: int, height: int, bgrx: bytes, step: int = 1) -> bytes:
    """A PNG of the frame with the standard library only (every step-th pixel),
    for hosts without Pillow (harbor's own Python)."""
    import zlib  # noqa: PLC0415

    w, h = width // step, height // step
    rows = bytearray()
    for y in range(0, h * step, step):
        rows.append(0)
        row = bgrx[y * width * 4 : (y + 1) * width * 4]
        px = row[0 : w * step * 4 : 1]
        for x in range(0, w * step, step):
            b, g, r = px[x * 4], px[x * 4 + 1], px[x * 4 + 2]
            rows += bytes((r, g, b))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
        )

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(rows), 6))
        + chunk(b"IEND", b"")
    )


def to_image(width: int, height: int, bgrx: bytes):  # type: ignore[no-untyped-def]
    from PIL import Image  # noqa: PLC0415 - Pillow only where screenshots are made

    return Image.frombuffer("RGB", (width, height), bgrx, "raw", "BGRX", 0, 1)
