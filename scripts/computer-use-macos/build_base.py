#!/usr/bin/env python3
"""Build the ComputerUse macOS base VM image from the pinned vanilla image.

    build_base.py [--name vbmac-base] [--source REF] [--out DIR]

Steps, all from the host (docs/DECISIONS.md, 2026-11-11):
 1. Clone the pinned vanilla image (Cirrus Labs macos-golden-gate-vanilla,
    macOS 27.2) and boot it headless with the VZ VNC server. One VNC
    connection is opened and held for the whole build: connecting while a
    macOS security prompt is on screen has crashed the VZ VNC server.
 2. Bootstrap Tart's guest agent by typing into Terminal over VNC: download
    the pinned release from github.com/openai/tart-guest-agent, verify its
    sha256, install it as a user LaunchAgent and a root LaunchDaemon (the
    layout Cirrus Labs' own images use).
 3. Grant the guest agent Automation of the apps tasks are seeded through
    (and data access where macOS asks for it) by sending one Apple event to
    each and clicking Allow on the prompt over VNC. The grants live in the
    user's TCC database; SIP stays on.
 4. Run base-config.sh in the guest (identity, time, updates, deterministic
    UI, a password the agent is never told, the vb-clock boot daemon).
 5. Dismiss each app's first-run sheets and iCloud prompts, clear
    notifications and the update badge, quit everything.
 6. Shut down through System Events (apps quit and save), so the image boots
    to an empty desktop.

The password comes from ~/.config/vulcanbench/macos-guest-password (created
if missing, mode 600) and is never printed. Screenshots of every step land in
--out for review. The result is a local Tart VM; nothing is published.
"""

from __future__ import annotations

import argparse
import hashlib
import secrets
import string
import subprocess
import sys
import threading
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))
from vb_macos import vm as V  # noqa: E402

SOURCE = (
    "ghcr.io/cirruslabs/macos-golden-gate-vanilla@sha256:"
    "b35695ce30453bcc1db961a476740be72a44c2a2e464b73ece1ff0fcac12a873"
)
AGENT_URL = (
    "https://github.com/openai/tart-guest-agent/releases/download/v0.15.0/"
    "tart-guest-agent-darwin-all.tar.gz"
)
AGENT_SHA256 = "eb47f402f18e742a8ea96344115ca776179f9c4c67494bc0c46a10a68db280b7"
PASSWORD_FILE = Path.home() / ".config" / "vulcanbench" / "macos-guest-password"
PLIST = (
    '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>'
    "<key>Label</key><string>{label}</string><key>ProgramArguments</key><array>"
    "<string>/usr/local/bin/tart-guest-agent</string><string>{arg}</string></array>"
    "<key>RunAtLoad</key><true/><key>KeepAlive</key><true/>"
    "<key>StandardOutPath</key><string>/tmp/{log}.log</string>"
    "<key>StandardErrorPath</key><string>/tmp/{log}.log</string></dict></plist>"
)
# One Apple event per app; the first one raises the Automation prompt.
GRANTS = [
    ("Notes", "count notes"),
    ("Reminders", "count lists"),
    ("Calendar", "count calendars"),
    ("Contacts", "count people"),
    ("Finder", "count windows"),
    ("System Events", "count processes"),
    ("Preview", "count documents"),
    ("TextEdit", "count documents"),
]
# Where Allow sits on the two kinds of prompt (1280x800 points; the prompts
# are centred on an otherwise empty screen when raised one at a time).
ALLOW_AUTOMATION = (699, 366)
ALLOW_DATA_ACCESS = (699, 290)


def password() -> str:
    if not PASSWORD_FILE.exists():
        PASSWORD_FILE.parent.mkdir(parents=True, exist_ok=True)
        alphabet = "".join(c for c in string.ascii_letters + string.digits if c not in "lIO01")
        PASSWORD_FILE.write_text("".join(secrets.choice(alphabet) for _ in range(20)))
        PASSWORD_FILE.chmod(0o600)
    return PASSWORD_FILE.read_text().strip()


class Builder:
    def __init__(self, name: str, out: Path):
        self.vm = V.VM(name, out)
        self.out = out
        self.step = 0

    def shot(self, label: str) -> None:
        self.step += 1
        self.vm.screenshot(self.out / f"{self.step:02d}-{label}.png")

    def click(self, x: int, y: int) -> None:
        self.vm.client.click(x * 2, y * 2)
        time.sleep(1.5)

    def osa(
        self, *lines: str, timeout: float = 60, check: bool = False
    ) -> subprocess.CompletedProcess[str]:
        args = " ".join("-e " + _sh(line) for line in lines)
        return self.vm.exec(f"osascript {args}", timeout=timeout, check=check)

    # 1 ------------------------------------------------------------------
    def boot(self, source: str) -> None:
        print(f"clone {source} -> {self.vm.name}", flush=True)
        self.vm.clone_from(source)
        self.vm.start()
        print(f"desktop after {self.vm.wait_desktop():.0f} s", flush=True)
        self.shot("booted")

    # 2 ------------------------------------------------------------------
    def bootstrap_agent(self) -> None:
        c = self.vm.client
        c.key("cmd+space")
        time.sleep(1.2)
        c.type_text("Terminal")
        time.sleep(1.5)
        c.key("return")
        time.sleep(5)
        agent = PLIST.format(
            label="org.cirruslabs.tart-guest-agent", arg="--run-agent", log="tart-guest-agent"
        )
        daemon = PLIST.format(
            label="org.cirruslabs.tart-guest-daemon", arg="--run-daemon", log="tart-guest-daemon"
        )
        cmd = (
            f"cd /tmp && curl -fsSLo tga.tgz {AGENT_URL} && "
            f'echo "{AGENT_SHA256}  tga.tgz" | shasum -a 256 -c && tar xzf tga.tgz && '
            "sudo mkdir -p /usr/local/bin && sudo cp /tmp/tart-guest-agent /usr/local/bin/ && "
            f"printf '%s' '{agent}' | sudo tee /Library/LaunchAgents/org.cirruslabs.tart-guest-agent.plist >/dev/null && "
            f"printf '%s' '{daemon}' | sudo tee /Library/LaunchDaemons/org.cirruslabs.tart-guest-daemon.plist >/dev/null && "
            "sudo launchctl bootstrap system /Library/LaunchDaemons/org.cirruslabs.tart-guest-daemon.plist; "
            "launchctl bootstrap gui/$(id -u) /Library/LaunchAgents/org.cirruslabs.tart-guest-agent.plist && "
            "echo AGENT-OK"
        )
        c.type_text(cmd)
        c.key("return")
        if not self.vm.agent_ready(timeout=180):
            self.shot("agent-failed")
            raise SystemExit("guest agent did not come up; see the screenshot")
        print("guest agent up", flush=True)
        self.shot("agent-up")

    # 3 ------------------------------------------------------------------
    def grant(self) -> None:
        for app, query in GRANTS:
            result: dict[str, subprocess.CompletedProcess[str]] = {}

            def send(app: str = app, query: str = query, result: dict = result) -> None:  # type: ignore[type-arg]
                result["r"] = self.osa(f'tell application "{app}" to {query}', timeout=120)

            t = threading.Thread(target=send)
            t.start()
            for attempt in range(8):
                t.join(timeout=5)
                if not t.is_alive():
                    break
                self.shot(f"grant-{app.replace(' ', '')}-{attempt}")
                self.click(*(ALLOW_AUTOMATION if attempt % 2 == 0 else ALLOW_DATA_ACCESS))
            t.join(timeout=60)
            r = result.get("r")
            ok = r is not None and r.returncode == 0
            print(f"grant {app}: {'ok' if ok else 'FAILED'}", flush=True)
            if not ok:
                raise SystemExit(f"could not grant {app}: {r.stderr if r else 'no result'}")

    # 4 ------------------------------------------------------------------
    def configure(self) -> None:
        script = (REPO / "scripts" / "computer-use-macos" / "base-config.sh").read_text()
        r = subprocess.run(
            [V.TART, "exec", self.vm.name, "/bin/sh", "-c", script, "base-config", password()],
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
        if "base-config-ok" not in r.stdout:
            raise SystemExit(f"base-config.sh failed: {r.stderr.strip()[-600:]}")
        print("base-config ok", flush=True)

    # 5 ------------------------------------------------------------------
    def first_runs(self) -> None:
        self.osa(
            'tell application "Terminal" to quit', 'tell application "Finder" to close every window'
        )
        self.vm.exec("open -a Notes")
        time.sleep(5)
        self.shot("notes-welcome")
        self.click(816, 651)  # Welcome to Notes: Continue
        time.sleep(2)
        self.shot("notes-icloud")
        self.click(640, 445)  # Turn On iCloud: Cancel
        self.osa('tell application "Notes" to quit')
        self.vm.exec("open -a Reminders")
        time.sleep(5)
        self.shot("reminders-welcome")
        self.click(504, 661)  # Welcome to Reminders: Continue
        time.sleep(2)
        self.click(504, 462)  # Enable iCloud Syncing?: Not Now
        self.click(439, 271)  # sidebar iCloud banner: close
        self.shot("reminders-done")
        self.osa('tell application "Reminders" to quit')
        for app in ("Calendar", "Contacts", "Preview", "TextEdit"):
            self.osa(f'tell application "{app}" to quit')
        self.osa(
            'tell application "Contacts"',
            "set c to my card",
            'set first name of c to "Jordan"',
            'set middle name of c to ""',
            'set last name of c to "Lee"',
            "save",
            "quit",
            "end tell",
        )
        self.vm.exec(
            "sudo -n defaults delete /Library/Preferences/com.apple.SoftwareUpdate RecommendedUpdates 2>/dev/null; "
            "sudo -n defaults write /Library/Preferences/com.apple.SoftwareUpdate LastRecommendedUpdatesAvailable -int 0; "
            "sudo -n defaults write /Library/Preferences/com.apple.SoftwareUpdate LastUpdatesAvailable -int 0; "
            "defaults write com.apple.systempreferences AttentionPrefBundleIDs -dict; "
            "sudo -n softwareupdate --clear-catalog >/dev/null 2>&1; killall Dock; true",
            check=False,
        )
        time.sleep(3)
        for _ in range(4):  # persistent notification banners: hover, close
            self.vm.client.move(1090 * 2, 90 * 2)
            time.sleep(1.2)
            self.click(924, 48)
        self.vm.client.move(640 * 2, 400 * 2)
        time.sleep(1)
        self.shot("clean-desktop")

    # 6 ------------------------------------------------------------------
    def finish(self) -> str:
        how = self.vm.shutdown()
        print(f"shutdown: {how}", flush=True)
        return how


def _sh(s: str) -> str:
    return "'" + s.replace("'", "'\"'\"'") + "'"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--name", default="vbmac-base")
    ap.add_argument("--source", default=SOURCE)
    ap.add_argument("--out", type=Path, default=Path.home() / "vulcanbench-gate" / "macos-build")
    args = ap.parse_args()
    out = args.out / args.name
    out.mkdir(parents=True, exist_ok=True)
    b = Builder(args.name, out)
    t0 = time.time()
    try:
        b.boot(args.source)
        b.bootstrap_agent()
        b.grant()
        b.configure()
        b.first_runs()
        how = b.finish()
    except BaseException:
        b.vm.stop()
        raise
    recipe = hashlib.sha256(
        Path(__file__).read_bytes()
        + (REPO / "scripts" / "computer-use-macos" / "base-config.sh").read_bytes()
    ).hexdigest()[:16]
    (out / "manifest.txt").write_text(
        f"vm={args.name}\nsource={args.source}\nrecipe_sha256_16={recipe}\nshutdown={how}\n"
        f"seconds={time.time() - t0:.0f}\n"
    )
    print(f"built {args.name} (recipe {recipe}) in {time.time() - t0:.0f} s", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
