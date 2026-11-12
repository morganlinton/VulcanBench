#!/usr/bin/env python3
"""VulcanBench computer tools over MCP (stdio), for agents that have no native
computer-use tool: Harbor's claude-code agent registers this server from the
task's task.toml and Claude Code calls the tools as mcp__computer__<name>.

The tools act on the same in-container desktop harbor's computer-1 agent
uses (Xvfb display :1, xdotool, ImageMagick `import`), so a run through this
server and a run through the native computer-use tool see the same screen.
Every action returns a fresh screenshot so the model sees its effect without
a second call. Coordinates are desktop pixels, origin top-left.

    python3 /opt/vbdesk/vb_computer_mcp.py [--start-url URL] [--condition NAME]

On start it brings the display up (sandbox/computer-use/ensure-display.sh,
a no-op if it is already running) with Chromium on --start-url. stdout is
the MCP transport; diagnostics go to stderr.

--condition (or VB_CONDITION) is the perception condition of the charter's
table (docs/COMPUTER_USE_CHARTER.md): "pixels+zoom" (default: full-size
screenshots and the zoom tool, the condition every reading so far was taken
under), "pixels" (the same screenshots, no zoom tool) or "pixels-lowres"
(screenshots scaled by VB_SCREENSHOT_SCALE, default 0.75, no zoom tool;
coordinates are taken in the scaled image and mapped back to the desktop).
scripts/computer-use/run_gate.sh sets it per run by copying the task with
the flag added to these args (CU_GATE_CONDITION).

--backend (or VB_BACKEND) picks what the tools drive: "x11" (default, the
Linux suite's in-container desktop) or "vnc" (the macOS suite: a Tart VM's
own VNC server, reached from harbor's container at the vnc:// URL the
environment writes to --vnc-url-file, default /run/vbmac/vnc.url; see
sandbox/computer-use/rfb.py). With vnc the desktop is the VM's 1280x800
point display (the Retina framebuffer is scaled down), keys take Mac names
(cmd, option, ctrl, shift), and every input event is appended to
VB_INPUT_LOG (default /var/lib/vbmac/input.log) as the ui_path evidence.
"""

from __future__ import annotations

import argparse
import io
import json
import os
import subprocess
import sys
import time
import urllib.request
from pathlib import Path
from typing import ClassVar

from mcp.server.mcpserver import Image, MCPServer

DISPLAY = ":1"
CONDITIONS = ("pixels+zoom", "pixels", "pixels-lowres")
BACKENDS = ("x11", "vnc")


def _flag_from_argv(name: str, env: str, default: str) -> str:
    """Read a flag before the server is built so the tool set can depend on it."""
    argv = sys.argv[1:]
    for i, a in enumerate(argv):
        if a == name and i + 1 < len(argv):
            return argv[i + 1]
        if a.startswith(name + "="):
            return a.split("=", 1)[1]
    return os.environ.get(env, default)


def _condition_from_argv() -> str:
    return _flag_from_argv("--condition", "VB_CONDITION", "pixels+zoom")


BACKEND_NAME = _flag_from_argv("--backend", "VB_BACKEND", "x11")
if BACKEND_NAME not in BACKENDS:
    print(
        f"vb_computer_mcp: unknown backend {BACKEND_NAME!r}; use one of {BACKENDS}", file=sys.stderr
    )
    sys.exit(2)
_DEFAULT_SIZE = ("1280", "800") if BACKEND_NAME == "vnc" else ("1024", "900")
WIDTH = int(os.environ.get("VB_DESKTOP_WIDTH", _DEFAULT_SIZE[0]))
HEIGHT = int(os.environ.get("VB_DESKTOP_HEIGHT", _DEFAULT_SIZE[1]))
CONDITION = _condition_from_argv()
if CONDITION not in CONDITIONS:
    print(
        f"vb_computer_mcp: unknown condition {CONDITION!r}; use one of {CONDITIONS}",
        file=sys.stderr,
    )
    sys.exit(2)
ZOOM_ENABLED = CONDITION == "pixels+zoom"
# Screenshots are scaled by this factor before they reach the model; tool
# coordinates arrive in the scaled image and are divided by it. 1.0 unless
# the condition is pixels-lowres.
SCALE = (
    float(os.environ.get("VB_SCREENSHOT_SCALE", "0.75")) if CONDITION == "pixels-lowres" else 1.0
)
if not 0.1 <= SCALE <= 1.0:
    print(
        f"vb_computer_mcp: VB_SCREENSHOT_SCALE must be in [0.1, 1.0], got {SCALE}", file=sys.stderr
    )
    sys.exit(2)
IMG_W, IMG_H = round(WIDTH * SCALE), round(HEIGHT * SCALE)
SETTLE_S = 0.4  # let the UI repaint before the post-action screenshot
MAX_WAIT_S = 10.0
WEBP_QUALITY = 80
BUTTONS = {"left": "1", "middle": "2", "right": "3"}
SCROLL_BUTTONS = {"up": "4", "down": "5", "left": "6", "right": "7"}

server = MCPServer(
    "computer",
    instructions=(
        f"Tools for operating the desktop shown in screenshots ({IMG_W}x{IMG_H} pixels, "
        "origin top-left; coordinates are pixels in that image). Take a screenshot first, then act "
        "with click, type, key, scroll or drag; every action returns the screen afterwards."
        + (" Use zoom to read small text." if ZOOM_ENABLED else "")
    ),
)


def log(msg: str) -> None:
    print(f"vb_computer_mcp: {msg}", file=sys.stderr, flush=True)


def run(argv: list[str], *, timeout: float = 30.0) -> subprocess.CompletedProcess[bytes]:
    env = {**os.environ, "DISPLAY": DISPLAY}
    return subprocess.run(argv, capture_output=True, timeout=timeout, env=env, check=False)


def xdo(*args: str) -> None:
    proc = run(["xdotool", *args])
    if proc.returncode != 0:
        raise RuntimeError(
            f"xdotool {' '.join(args)} failed: {proc.stderr.decode(errors='replace').strip()}"
        )


def clamp(x: int, y: int) -> tuple[int, int]:
    """Image coordinates from the model -> desktop pixels, clamped to the screen."""
    dx, dy = round(int(x) / SCALE), round(int(y) / SCALE)
    return max(0, min(WIDTH - 1, dx)), max(0, min(HEIGHT - 1, dy))


def shown(x: int, y: int) -> str:
    """A desktop point as the model sees it (image coordinates), for tool notes."""
    return f"({round(x * SCALE)},{round(y * SCALE)})"


def to_webp(png: bytes) -> bytes:
    """Encode like computer-1 does (cwebp, quality 80): a text page is a few
    tens of KB instead of about 150 KB as PNG, which keeps long runs in context."""
    proc = subprocess.run(
        ["cwebp", "-quiet", "-q", str(WEBP_QUALITY), "-o", "-", "--", "-"],
        input=png,
        capture_output=True,
        timeout=30,
        check=False,
    )
    if proc.returncode != 0 or not proc.stdout:
        return png
    return proc.stdout


class X11Backend:
    """The Linux suite's in-container desktop: Xvfb :1, xdotool, ImageMagick."""

    def grab(self, region: tuple[int, int, int, int] | None = None, scale: float = 1) -> bytes:
        argv = ["import", "-window", "root"]
        if region:
            x0, y0, x1, y1 = region
            argv += ["-crop", f"{x1 - x0}x{y1 - y0}+{x0}+{y0}", "+repage"]
        if region is None and SCALE != 1.0:
            scale = SCALE  # the condition's downscale of every full screenshot
        if scale != 1:
            filt = "point" if scale > 1 else "triangle"
            argv += [
                "-filter",
                filt,
                "-resize",
                f"{IMG_W}x{IMG_H}!" if region is None else f"{scale * 100}%",
            ]
        argv.append("png:-")
        proc = run(argv)
        if proc.returncode != 0 or not proc.stdout:
            raise RuntimeError(f"screenshot failed: {proc.stderr.decode(errors='replace').strip()}")
        return to_webp(proc.stdout)

    def click(self, x: int, y: int, button: str, count: int) -> None:
        code = BUTTONS.get(button, "1")
        if count == 1:
            xdo("mousemove", "--sync", str(x), str(y), "click", code)
        else:
            xdo(
                "mousemove",
                "--sync",
                str(x),
                str(y),
                "click",
                "--repeat",
                str(count),
                "--delay",
                "80",
                code,
            )

    def move(self, x: int, y: int) -> None:
        xdo("mousemove", "--sync", str(x), str(y))

    def type_text(self, text: str) -> None:
        xdo("type", "--delay", "12", "--", text)

    def key(self, name: str) -> None:
        xdo("key", "--clearmodifiers", name)

    def scroll(self, x: int, y: int, direction: str, amount: int) -> None:
        code = SCROLL_BUTTONS.get(direction, "5")
        xdo(
            "mousemove",
            "--sync",
            str(x),
            str(y),
            "click",
            "--repeat",
            str(amount),
            "--delay",
            "30",
            code,
        )

    def drag(self, x0: int, y0: int, x1: int, y1: int) -> None:
        xdo("mousemove", "--sync", str(x0), str(y0), "mousedown", "1")
        xdo("mousemove", "--sync", str(x1), str(y1))
        xdo("mouseup", "1")

    def cursor(self) -> tuple[int, int] | None:
        proc = run(["xdotool", "getmouselocation", "--shell"])
        vals = dict(
            line.split("=", 1)
            for line in proc.stdout.decode(errors="replace").splitlines()
            if "=" in line
        )
        try:
            return int(vals["X"]), int(vals["Y"])
        except (KeyError, ValueError):
            return None

    def start(self, start_url: str | None) -> None:
        env = dict(os.environ)
        if start_url:
            env["VB_START_URL"] = start_url
        env.setdefault("VB_DESKTOP_WIDTH", str(WIDTH))
        env.setdefault("VB_DESKTOP_HEIGHT", str(HEIGHT))
        proc = subprocess.run(
            ["bash", "/opt/vbdesk/ensure-display.sh"],
            env=env,
            capture_output=True,
            text=True,
            timeout=240,
            check=False,
        )
        log(proc.stdout.strip() or proc.stderr.strip())
        if proc.returncode != 0:
            log("display bring-up failed; tools will report errors")


class VNCBackend:
    """A Tart macOS VM through its own VNC server (macOS suite).

    Desktop coordinates are the VM's 1280x800 points; the framebuffer is
    the Retina 2560x1600, so input is scaled up by `k` and screenshots down.
    One RFB connection is held for the whole session (rfb.py explains why).
    """

    BUTTON_NUMBERS: ClassVar[dict[str, int]] = {"left": 1, "middle": 2, "right": 3}

    def __init__(self) -> None:
        self.url_file = Path(
            _flag_from_argv("--vnc-url-file", "VB_VNC_URL_FILE", "/run/vbmac/vnc.url")
        )
        self.input_log = Path(os.environ.get("VB_INPUT_LOG", "/var/lib/vbmac/input.log"))
        self.client = None  # rfb.RFBClient, after start()
        self.k = 2.0
        self.t0 = time.time()

    def _note(self, kind: str, **args: object) -> None:
        try:
            self.input_log.parent.mkdir(parents=True, exist_ok=True)
            with self.input_log.open("a") as f:
                f.write(json.dumps({"t": round(time.time(), 3), "kind": kind, **args}) + "\n")
        except OSError as exc:
            log(f"input log write failed: {exc}")

    def start(self, start_url: str | None) -> None:
        import rfb  # noqa: PLC0415 - next to this file; the X11 image does not need it

        url = os.environ.get("VB_VNC_URL")
        deadline = time.time() + 600
        while not url and time.time() < deadline:
            if self.url_file.is_file() and self.url_file.read_text().strip():
                url = self.url_file.read_text().strip()
                break
            time.sleep(1)
        if not url:
            log(f"no VNC URL in {self.url_file}; tools will report errors")
            return
        url = url.replace(
            "@127.0.0.1:", f"@{os.environ.get('VB_VNC_HOST', 'host.docker.internal')}:"
        )
        self.client = rfb.RFBClient(url)
        for attempt in range(30):
            try:
                self.client.connect()
                break
            except (OSError, rfb.RFBError) as exc:
                log(f"VNC connect attempt {attempt + 1} failed: {exc}")
                time.sleep(2)
        if self.client.width:
            self.k = self.client.width / WIDTH
        log(f"VNC framebuffer {self.client.width}x{self.client.height}, scale {self.k:g}")

    def _c(self):  # type: ignore[no-untyped-def]
        if self.client is None:
            raise RuntimeError("the VM display is not connected")
        return self.client

    def _fb(self, x: int, y: int) -> tuple[int, int]:
        return round(x * self.k), round(y * self.k)

    def grab(self, region: tuple[int, int, int, int] | None = None, scale: float = 1) -> bytes:
        import rfb  # noqa: PLC0415
        from PIL import Image as PILImage  # noqa: PLC0415 - only the macOS image has Pillow

        w, h, data = self._c().capture()
        im = rfb.to_image(w, h, data)
        if region:
            x0, y0, x1, y1 = region
            im = im.crop(
                (round(x0 * self.k), round(y0 * self.k), round(x1 * self.k), round(y1 * self.k))
            )
            size = (round((x1 - x0) * scale), round((y1 - y0) * scale))
        else:
            size = (IMG_W, IMG_H)
        if im.size != size:
            im = im.resize(size, PILImage.Resampling.LANCZOS)
        buf = io.BytesIO()
        im.save(buf, format="WEBP", quality=WEBP_QUALITY)
        return buf.getvalue()

    def click(self, x: int, y: int, button: str, count: int) -> None:
        self._note("click", x=x, y=y, button=button, count=count)
        self._c().click(*self._fb(x, y), button=self.BUTTON_NUMBERS.get(button, 1), count=count)

    def move(self, x: int, y: int) -> None:
        self._note("move", x=x, y=y)
        self._c().move(*self._fb(x, y))

    def type_text(self, text: str) -> None:
        self._note("type", chars=len(text))
        self._c().type_text(text)

    def key(self, name: str) -> None:
        self._note("key", key=name)
        self._c().key(name)

    def scroll(self, x: int, y: int, direction: str, amount: int) -> None:
        """The VZ VNC server carries no wheel events, so the pointer is moved
        over VNC and the wheel is turned in the guest through the
        environment's scroll endpoint (vbscroll). One wheel click is three
        lines (30 points), about what a mouse wheel moves."""
        self._note("scroll", x=x, y=y, direction=direction, amount=amount)
        self._c().move(*self._fb(x, y))
        time.sleep(0.15)
        lines = 3 * amount
        dy = lines if direction == "up" else -lines if direction == "down" else 0
        dx = lines if direction == "left" else -lines if direction == "right" else 0
        url_file = Path(os.environ.get("VB_SCROLL_URL_FILE", "/run/vbmac/scroll.url"))
        if not url_file.is_file():
            raise RuntimeError("scrolling is unavailable: no scroll endpoint for this VM")
        url = url_file.read_text().strip() + f"&dy={dy}&dx={dx}"
        with urllib.request.urlopen(url, timeout=40) as resp:
            if resp.status != 200:
                raise RuntimeError(f"scroll failed: {resp.read()[:200]!r}")

    def drag(self, x0: int, y0: int, x1: int, y1: int) -> None:
        self._note("drag", x0=x0, y0=y0, x1=x1, y1=y1)
        c = self._c()
        fx0, fy0 = self._fb(x0, y0)
        fx1, fy1 = self._fb(x1, y1)
        c.button(fx0, fy0, 1, True)
        time.sleep(0.15)
        steps = 8
        for i in range(1, steps + 1):  # intermediate moves so drag-and-drop registers
            c.move(round(fx0 + (fx1 - fx0) * i / steps), round(fy0 + (fy1 - fy0) * i / steps))
            time.sleep(0.03)
        time.sleep(0.15)
        c.button(fx1, fy1, 1, False)

    def cursor(self) -> tuple[int, int] | None:
        if self.client is None:
            return None
        px, py = self.client.pointer
        return round(px / self.k), round(py / self.k)


BACKEND = VNCBackend() if BACKEND_NAME == "vnc" else X11Backend()
KEY_DOC = (
    "Press one key or chord, Mac names: return, tab, escape, delete (backspace), space, "
    "left, right, up, down, pagedown, home, end, f1-f12, or chords such as cmd+c, cmd+shift+4, "
    "option+left, ctrl+a. Separate several presses with spaces."
    if BACKEND_NAME == "vnc"
    else "Press one key or chord, xdotool names: Return, Tab, Escape, BackSpace, Page_Down, "
    "ctrl+l, ctrl+a, alt+Left. Separate several presses with spaces."
)


def grab(region: tuple[int, int, int, int] | None = None, scale: float = 1) -> bytes:
    return BACKEND.grab(region, scale)


def image(data: bytes) -> Image:
    return Image(data=data, format="webp" if data[:4] == b"RIFF" else "png")


def observe(note: str) -> list[Image | str]:
    time.sleep(SETTLE_S)
    return [image(grab()), note]


@server.tool()
def screenshot() -> list[Image | str]:
    """Capture the current screen."""
    return [image(grab()), f"Screen {IMG_W}x{IMG_H}, origin top-left."]


def zoom(x0: int, y0: int, x1: int, y1: int) -> list[Image | str]:
    """Capture the region from (x0, y0) to (x1, y1) at double size, to read small text. Coordinates in the result are not screen coordinates."""
    x0, y0 = clamp(x0, y0)
    x1, y1 = clamp(x1, y1)
    if x1 - x0 < 8 or y1 - y0 < 8:
        return ["Zoom region must be at least 8x8 pixels."]
    return [
        image(grab((x0, y0, x1, y1), scale=2)),
        f"Region {shown(x0, y0)}-{shown(x1, y1)} at 2x.",
    ]


if ZOOM_ENABLED:
    server.tool()(zoom)


@server.tool()
def click(x: int, y: int, button: str = "left") -> list[Image | str]:
    """Move the mouse to (x, y) and click once. button: left, middle or right."""
    x, y = clamp(x, y)
    BACKEND.click(x, y, button, 1)
    return observe(f"Clicked {button} at {shown(x, y)}.")


@server.tool()
def double_click(x: int, y: int) -> list[Image | str]:
    """Move the mouse to (x, y) and double-click the left button."""
    x, y = clamp(x, y)
    BACKEND.click(x, y, "left", 2)
    return observe(f"Double-clicked at {shown(x, y)}.")


@server.tool()
def triple_click(x: int, y: int) -> list[Image | str]:
    """Move the mouse to (x, y) and triple-click, which selects a line of text."""
    x, y = clamp(x, y)
    BACKEND.click(x, y, "left", 3)
    return observe(f"Triple-clicked at {shown(x, y)}.")


@server.tool()
def mouse_move(x: int, y: int) -> list[Image | str]:
    """Move the mouse to (x, y) without clicking (for hover states)."""
    x, y = clamp(x, y)
    BACKEND.move(x, y)
    return observe(f"Mouse at {shown(x, y)}.")


@server.tool()
def type_text(text: str) -> list[Image | str]:
    """Type text into the focused element, as on a keyboard."""
    if not text:
        return ["Nothing to type."]
    try:
        BACKEND.type_text(text)
    except ValueError as exc:
        return [str(exc)]
    return observe(f"Typed {len(text)} characters.")


def key(keys: str) -> list[Image | str]:
    names = keys.split()
    if not names:
        return ["No key given."]
    for name in names:
        try:
            BACKEND.key(name)
        except ValueError as exc:
            return [str(exc)]
    return observe(f"Pressed {' '.join(names)}.")


key.__doc__ = KEY_DOC
server.tool()(key)


@server.tool()
def scroll(x: int, y: int, direction: str = "down", amount: int = 3) -> list[Image | str]:
    """Scroll at (x, y). direction: up, down, left or right; amount: wheel clicks (1 to 20)."""
    x, y = clamp(x, y)
    if direction not in SCROLL_BUTTONS:
        direction = "down"
    amount = max(1, min(20, int(amount)))
    BACKEND.scroll(x, y, direction, amount)
    return observe(f"Scrolled {direction} {amount} at {shown(x, y)}.")


@server.tool()
def drag(x0: int, y0: int, x1: int, y1: int) -> list[Image | str]:
    """Press the left button at (x0, y0), move to (x1, y1) and release."""
    x0, y0 = clamp(x0, y0)
    x1, y1 = clamp(x1, y1)
    BACKEND.drag(x0, y0, x1, y1)
    return observe(f"Dragged {shown(x0, y0)} to {shown(x1, y1)}.")


@server.tool()
def wait(seconds: float = 1.0) -> list[Image | str]:
    """Wait up to 10 seconds for the screen to change, then capture it."""
    seconds = max(0.0, min(MAX_WAIT_S, float(seconds)))
    time.sleep(seconds)
    return [image(grab()), f"Waited {seconds:.1f}s."]


@server.tool()
def cursor_position() -> str:
    """Report the current mouse position, in screenshot coordinates."""
    pos = BACKEND.cursor()
    if pos is None:
        return "unknown"
    return f"X={round(pos[0] * SCALE)} Y={round(pos[1] * SCALE)}"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument(
        "--start-url", default=None, help="page Chromium opens when the display is started here"
    )
    ap.add_argument(
        "--condition",
        default=CONDITION,
        choices=CONDITIONS,
        help="perception condition (default pixels+zoom; see the module docstring)",
    )
    ap.add_argument("--backend", default=BACKEND_NAME, choices=BACKENDS)
    ap.add_argument("--vnc-url-file", default=None, help="vnc backend: file holding the vnc:// URL")
    args = ap.parse_args()
    log(
        f"backend {BACKEND_NAME}, condition {CONDITION}: screenshots {IMG_W}x{IMG_H}, "
        f"zoom {'on' if ZOOM_ENABLED else 'off'}"
    )
    BACKEND.start(args.start_url)
    server.run(transport="stdio")


if __name__ == "__main__":
    main()
