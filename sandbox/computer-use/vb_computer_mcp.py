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
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time

from mcp.server.mcpserver import Image, MCPServer

DISPLAY = ":1"
WIDTH = int(os.environ.get("VB_DESKTOP_WIDTH", "1024"))
HEIGHT = int(os.environ.get("VB_DESKTOP_HEIGHT", "900"))
CONDITIONS = ("pixels+zoom", "pixels", "pixels-lowres")


def _condition_from_argv() -> str:
    """--condition is read before the server is built so the tool set can depend on it."""
    argv = sys.argv[1:]
    for i, a in enumerate(argv):
        if a == "--condition" and i + 1 < len(argv):
            return argv[i + 1]
        if a.startswith("--condition="):
            return a.split("=", 1)[1]
    return os.environ.get("VB_CONDITION", "pixels+zoom")


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


def grab(region: tuple[int, int, int, int] | None = None, scale: float = 1) -> bytes:
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
    code = BUTTONS.get(button, "1")
    xdo("mousemove", "--sync", str(x), str(y), "click", code)
    return observe(f"Clicked {button} at {shown(x, y)}.")


@server.tool()
def double_click(x: int, y: int) -> list[Image | str]:
    """Move the mouse to (x, y) and double-click the left button."""
    x, y = clamp(x, y)
    xdo("mousemove", "--sync", str(x), str(y), "click", "--repeat", "2", "--delay", "80", "1")
    return observe(f"Double-clicked at {shown(x, y)}.")


@server.tool()
def triple_click(x: int, y: int) -> list[Image | str]:
    """Move the mouse to (x, y) and triple-click, which selects a line of text."""
    x, y = clamp(x, y)
    xdo("mousemove", "--sync", str(x), str(y), "click", "--repeat", "3", "--delay", "80", "1")
    return observe(f"Triple-clicked at {shown(x, y)}.")


@server.tool()
def mouse_move(x: int, y: int) -> list[Image | str]:
    """Move the mouse to (x, y) without clicking (for hover states)."""
    x, y = clamp(x, y)
    xdo("mousemove", "--sync", str(x), str(y))
    return observe(f"Mouse at {shown(x, y)}.")


@server.tool()
def type_text(text: str) -> list[Image | str]:
    """Type text into the focused element, as on a keyboard."""
    if not text:
        return ["Nothing to type."]
    xdo("type", "--delay", "12", "--", text)
    return observe(f"Typed {len(text)} characters.")


@server.tool()
def key(keys: str) -> list[Image | str]:
    """Press one key or chord, xdotool names: Return, Tab, Escape, BackSpace, Page_Down, ctrl+l, ctrl+a, alt+Left. Separate several presses with spaces."""
    names = keys.split()
    if not names:
        return ["No key given."]
    for name in names:
        xdo("key", "--clearmodifiers", name)
    return observe(f"Pressed {' '.join(names)}.")


@server.tool()
def scroll(x: int, y: int, direction: str = "down", amount: int = 3) -> list[Image | str]:
    """Scroll at (x, y). direction: up, down, left or right; amount: wheel clicks (1 to 20)."""
    x, y = clamp(x, y)
    code = SCROLL_BUTTONS.get(direction, "5")
    amount = max(1, min(20, int(amount)))
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
    return observe(f"Scrolled {direction} {amount} at {shown(x, y)}.")


@server.tool()
def drag(x0: int, y0: int, x1: int, y1: int) -> list[Image | str]:
    """Press the left button at (x0, y0), move to (x1, y1) and release."""
    x0, y0 = clamp(x0, y0)
    x1, y1 = clamp(x1, y1)
    xdo("mousemove", "--sync", str(x0), str(y0), "mousedown", "1")
    xdo("mousemove", "--sync", str(x1), str(y1))
    xdo("mouseup", "1")
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
    proc = run(["xdotool", "getmouselocation", "--shell"])
    vals = dict(
        line.split("=", 1)
        for line in proc.stdout.decode(errors="replace").splitlines()
        if "=" in line
    )
    try:
        return f"X={round(int(vals['X']) * SCALE)} Y={round(int(vals['Y']) * SCALE)}"
    except (KeyError, ValueError):
        return "unknown"


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
    args = ap.parse_args()
    log(
        f"condition {CONDITION}: screenshots {IMG_W}x{IMG_H}, zoom {'on' if ZOOM_ENABLED else 'off'}"
    )
    env = dict(os.environ)
    if args.start_url:
        env["VB_START_URL"] = args.start_url
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
    server.run(transport="stdio")


if __name__ == "__main__":
    main()
