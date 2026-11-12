"""One Tart macOS VM: clone, boot headless with the VZ VNC server, exec through
the guest agent, shut down cleanly, and read its disk read-only.

Findings this module encodes (docs/DECISIONS.md, 2026-11-11):
- `tart run --vnc-experimental` prints `vnc://:PASSWORD@127.0.0.1:PORT`; the
  server answers one framebuffer request per connection (rfb.py copes).
- Host to guest TCP is blocked for processes launched from some apps (Local
  Network privacy), so nothing here uses SSH: commands go through Tart's guest
  agent (`tart exec`, a virtio socket) and state is read from the disk.
- `tart stop` is a hard power-off; apps lose unsaved state. `shutdown()`
  asks loginwindow for a normal shutdown first, which quits apps gracefully.
- The disk image is ASIF: attach with `diskutil image attach`, not raw hdiutil.
"""

from __future__ import annotations

import contextlib
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from collections.abc import Iterator
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "sandbox" / "computer-use"))
import rfb  # noqa: E402

TART = os.environ.get("TART") or shutil.which("tart") or "/opt/homebrew/bin/tart"
VNC_RE = re.compile(r"vnc://[^\s]+")


def tart(*args: str, timeout: float = 600, check: bool = True) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        [TART, *args], capture_output=True, text=True, timeout=timeout, check=False
    )
    if check and proc.returncode != 0:
        raise RuntimeError(f"tart {' '.join(args)} failed: {(proc.stderr or proc.stdout).strip()}")
    return proc


def vm_state(name: str) -> str | None:
    try:
        for row in json.loads(tart("list", "--format", "json").stdout):
            if row.get("Name") == name:
                return str(row.get("State") or row.get("Running"))
    except (ValueError, RuntimeError):
        return None
    return None


class VM:
    def __init__(self, name: str, log_dir: Path):
        self.name = name
        self.log_dir = Path(log_dir)
        self.log_dir.mkdir(parents=True, exist_ok=True)
        self.proc: subprocess.Popen[bytes] | None = None
        self.vnc_url: str | None = None
        self._client: rfb.RFBClient | None = None

    # -- lifecycle ------------------------------------------------------------
    def clone_from(self, source: str) -> None:
        if vm_state(self.name) is not None:
            tart("delete", self.name, check=False)
        tart("clone", source, self.name, timeout=1800)

    def start(self, display: str = "1280x800", extra: tuple[str, ...] = ()) -> str:
        tart("set", self.name, "--display", display)
        log = open(self.log_dir / f"{self.name}.run.log", "wb")  # noqa: SIM115
        self.proc = subprocess.Popen(
            [
                TART,
                "run",
                "--no-graphics",
                "--no-audio",
                "--no-clipboard",
                "--vnc-experimental",
                *extra,
                self.name,
            ],
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        deadline = time.time() + 180
        path = self.log_dir / f"{self.name}.run.log"
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise RuntimeError(
                    f"tart run exited early: {path.read_text(errors='replace')[-500:]}"
                )
            m = VNC_RE.search(path.read_text(errors="replace"))
            if m:
                self.vnc_url = m.group(0).rstrip(".,")
                return self.vnc_url
            time.sleep(0.5)
        raise RuntimeError("no VNC URL from tart run within 180 s")

    @property
    def client(self) -> rfb.RFBClient:
        if self._client is None:
            assert self.vnc_url, "start() first"
            self._client = rfb.RFBClient(self.vnc_url)
            self._client.connect()
        return self._client

    def screenshot(self, path: Path | None = None, size: tuple[int, int] = (1280, 800)):  # type: ignore[no-untyped-def]
        """Save (and return, when Pillow is present) the screen at `size`."""
        w, h, data = self.client.capture()
        try:
            from PIL import Image  # noqa: F401, PLC0415 - harbor's Python has no Pillow
        except ImportError:
            if path:
                Path(path).write_bytes(rfb.png_bytes(w, h, data, step=max(1, w // size[0])))
            return None
        im = rfb.to_image(w, h, data)
        if size and im.size != size:
            im = im.resize(size)
        if path:
            im.save(path)
        return im

    def wait_desktop(self, timeout: float = 300, settle: float = 8.0) -> float:
        """Until the menu bar is drawn and two captures `settle` s apart match.
        Works on the raw framebuffer, so it needs no Pillow."""
        t0 = time.time()
        prev = None
        while time.time() - t0 < timeout:
            try:
                w, h, data = self.client.capture()
            except (OSError, rfb.RFBError):
                time.sleep(2)
                self.close_client()
                continue
            bar_rows = max(1, h // 64)  # the menu bar's top strip
            strip = data[: w * 4 * bar_rows]
            sample = strip[::64]
            bright = sum(sample) / max(1, len(sample))
            if bright > 60:
                sig = hashlib.sha256(data[:: 4 * 97]).digest()
                if prev is not None and prev == sig:
                    return time.time() - t0
                prev = sig
                time.sleep(settle)
                continue
            time.sleep(2)
        raise RuntimeError(f"desktop not ready within {timeout} s")

    def exec(
        self, cmd: str, timeout: float = 120, check: bool = True
    ) -> subprocess.CompletedProcess[str]:
        """Run a shell command in the guest as the logged-in user (guest agent)."""
        proc = subprocess.run(
            [TART, "exec", self.name, "/bin/sh", "-c", cmd],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        if check and proc.returncode != 0:
            raise RuntimeError(
                f"guest command failed ({proc.returncode}): {cmd[:120]}\n{proc.stderr.strip()[-800:]}"
            )
        return proc

    def agent_ready(self, timeout: float = 120) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                if self.exec("true", timeout=15, check=False).returncode == 0:
                    return True
            except subprocess.TimeoutExpired:
                pass
            time.sleep(2)
        return False

    def shutdown(self, timeout: float = 120) -> str:
        """A normal macOS shutdown (apps quit and save), then wait for the VM to stop.
        Returns how it went down: "loginwindow", "sudo" or "forced"."""
        how = "forced"
        if self.agent_ready(timeout=20):
            # System Events quits every app gracefully (they save) with no
            # confirmation dialog; the base image grants the guest agent
            # Automation of System Events. loginwindow's shutdown event works
            # too but waits out its 60 s "Are you sure" countdown.
            r = self.exec(
                "osascript -e 'tell application \"System Events\" to shut down'",
                timeout=60,
                check=False,
            )
            how = "system-events" if r.returncode == 0 else how
            if how == "forced":
                r = self.exec(
                    "osascript -e 'tell application \"loginwindow\" to «event aevtrsdn»'",
                    timeout=30,
                    check=False,
                )
                how = "loginwindow" if r.returncode == 0 else how
            if how == "forced":
                r = self.exec("sudo -n shutdown -h now", timeout=30, check=False)
                how = "sudo" if r.returncode == 0 else how
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.proc is not None and self.proc.poll() is not None:
                break
            if self.proc is None and vm_state(self.name) in ("stopped", "False"):
                break
            time.sleep(1)
        else:
            how = "forced"
        if self.proc is not None and self.proc.poll() is None:
            tart("stop", self.name, check=False)
            self.proc.wait(timeout=60)
            how = "forced"
        self.close_client()
        return how

    def stop(self) -> None:
        tart("stop", self.name, check=False)
        if self.proc is not None:
            with contextlib.suppress(subprocess.TimeoutExpired):
                self.proc.wait(timeout=60)
        self.close_client()

    def close_client(self) -> None:
        if self._client is not None:
            self._client.close()
            self._client = None

    def delete(self) -> None:
        self.stop()
        tart("delete", self.name, check=False)

    def disk_path(self) -> Path:
        return Path.home() / ".tart" / "vms" / self.name / "disk.img"


@contextlib.contextmanager
def mounted_data(disk: Path, mountpoint: Path, *, readonly: bool = True) -> Iterator[Path]:
    """Attach a stopped VM's disk and mount its APFS Data volume; always detach."""
    args = ["diskutil", "image", "attach", "--noMount"] + (["--readOnly"] if readonly else [])
    out = subprocess.run(
        [*args, str(disk)], capture_output=True, text=True, timeout=120, check=True
    ).stdout
    whole = re.sub(r"s\d+$", "", out.split()[0]) if out.split() else ""
    try:
        data_vol = None
        for line in out.splitlines():
            dev = line.split()[0] if line.split() else ""
            if "Apple_APFS_Volume" in line or re.match(r"/dev/disk\d+s\d+$", dev):
                info = subprocess.run(
                    ["diskutil", "info", dev],
                    capture_output=True,
                    text=True,
                    timeout=30,
                    check=False,
                ).stdout
                if re.search(r"Volume Name:\s+Data\s*$", info, re.M):
                    data_vol = dev
                    break
        if data_vol is None:
            raise RuntimeError(f"no Data volume on {disk}")
        mountpoint.mkdir(parents=True, exist_ok=True)
        mode = ["readOnly"] if readonly else []
        subprocess.run(
            ["diskutil", "mount", *mode, "-mountPoint", str(mountpoint), data_vol],
            capture_output=True,
            text=True,
            timeout=120,
            check=True,
        )
        try:
            yield mountpoint
        finally:
            subprocess.run(
                ["diskutil", "unmount", str(mountpoint)],
                capture_output=True,
                timeout=120,
                check=False,
            )
    finally:
        if whole:
            subprocess.run(
                ["hdiutil", "detach", whole], capture_output=True, timeout=120, check=False
            )
