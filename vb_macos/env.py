"""Harbor environment for the ComputerUse macOS suite: a Tart VM per trial,
next to harbor's ordinary Docker container for the agent.

    harbor run --env vb_macos.env:TartEnvironment ...   (PYTHONPATH=<repo>)

The agent (Claude Code or Codex) and the suite's MCP computer tools run in
the task's Docker container exactly as in the Linux suite, so harbor's agent
install, auth, MCP wiring, logs and network policy are unchanged. What this
class adds:

- start(): clone the task's VM image (named in environment/vm.json), boot it
  headless with the VZ VNC server, wait for its desktop, then start the
  container and write the VNC URL to /run/vbmac/vnc.url in it (outside the graded artifacts, since it holds the VNC password). The MCP
  server (--backend vnc) reads that file and connects to the VM through
  host.docker.internal.
- service_exec("vbmac-collect PATH..."): the task's collect hook. Shut the
  guest down cleanly (apps quit and save), mount its disk read-only on the
  host, copy the listed paths (relative to the guest user's home, or
  absolute under /) and upload them into the container at
  /var/lib/vbmac/export/fs, with a manifest. The checker reads them there.
- stop(): stop the container, stop and delete the clone, detach any disk.

environment/vm.json: {"image": "vbmac-task-<slug>", "display": "1280x800"}.
The image is a local Tart VM built by scripts/computer-use-macos/
build_task_image.py; trials never modify it.
"""

from __future__ import annotations

import asyncio
import json
import shlex
import shutil
import subprocess
import tempfile
import time
import uuid
from pathlib import Path
from typing import Any

from harbor.environments.docker.docker import DockerEnvironment

from vb_macos import vm as V

SENTINEL = "vbmac-collect"
GUEST_HOME = "Users/admin"
EXPORT = "/var/lib/vbmac/export"


class TartEnvironment(DockerEnvironment):
    def __init__(self, *args: Any, **kwargs: Any) -> None:
        super().__init__(*args, **kwargs)
        # The task's agent environment names its VM in environment/vm.json. A
        # separate verifier environment (tests/) is built from this class too
        # and has none: it is then a plain Docker environment.
        spec_path = Path(self.environment_dir) / "vm.json"
        self.vm_spec: dict[str, Any] | None = (
            json.loads(spec_path.read_text()) if spec_path.is_file() else None
        )
        self.vm_name = f"vbmac-trial-{uuid.uuid4().hex[:10]}"
        self.vm_logs = Path(self.trial_paths.trial_dir) / "vm"
        self.vm: V.VM | None = None
        self.vm_events: list[dict[str, Any]] = []

    # -- helpers -------------------------------------------------------------
    def _event(self, what: str, **info: Any) -> None:
        self.vm_events.append({"t": round(time.time(), 3), "event": what, **info})
        self.vm_logs.mkdir(parents=True, exist_ok=True)
        (self.vm_logs / "events.json").write_text(json.dumps(self.vm_events, indent=1))

    def _boot_vm(self) -> str:
        assert self.vm_spec is not None
        image = self.vm_spec["image"]
        self.vm = V.VM(self.vm_name, self.vm_logs)
        t0 = time.time()
        self.vm.clone_from(image)
        url = self.vm.start(display=self.vm_spec.get("display", "1280x800"))
        ready = self.vm.wait_desktop(timeout=float(self.vm_spec.get("boot_timeout_sec", 300)))
        self.vm.screenshot(self.vm_logs / "start.png")
        # Free the VNC connection for the agent's MCP server: one client at a
        # time keeps the VZ server away from its connect-time crash.
        self.vm.close_client()
        self._event(
            "booted", image=image, seconds=round(time.time() - t0, 1), desktop_s=round(ready, 1)
        )
        return url

    # -- lifecycle -----------------------------------------------------------
    async def start(self, force_build: bool) -> None:
        if self.vm_spec is None:
            await super().start(force_build)
            return
        url = await asyncio.to_thread(self._boot_vm)
        try:
            await super().start(force_build)
            quoted = shlex.quote(url)
            res = await super().exec(
                f"mkdir -p /run/vbmac /var/lib/vbmac && umask 077 && printf %s {quoted} > /run/vbmac/vnc.url",
                timeout_sec=30,
            )
            if res.return_code != 0:
                raise RuntimeError(f"could not hand the VNC URL to the container: {res.stderr}")
        except BaseException:
            await asyncio.to_thread(self._delete_vm)
            raise

    def _collect(self, paths: list[str]) -> tuple[Path, dict[str, Any]]:
        assert self.vm is not None and self.vm_spec is not None
        t0 = time.time()
        how = self.vm.shutdown(timeout=float(self.vm_spec.get("shutdown_timeout_sec", 120)))
        out = Path(tempfile.mkdtemp(prefix="vbmac-collect-"))
        fs = out / "fs"
        fs.mkdir()
        manifest: dict[str, Any] = {"shutdown": how, "paths": {}}
        mnt = Path(tempfile.mkdtemp(prefix="vbmac-mnt-"))
        with V.mounted_data(self.vm.disk_path(), mnt, readonly=True) as root:
            for p in paths:
                rel = p.lstrip("/") if p.startswith("/") else f"{GUEST_HOME}/{p}"
                src = root / rel
                dst = fs / rel
                if not src.exists():
                    manifest["paths"][p] = "missing"
                    continue
                dst.parent.mkdir(parents=True, exist_ok=True)
                if src.is_dir():
                    shutil.copytree(src, dst, symlinks=True, ignore_dangling_symlinks=True)
                else:
                    shutil.copy2(src, dst)
                manifest["paths"][p] = "copied"
        manifest["seconds"] = round(time.time() - t0, 1)
        (out / "manifest.json").write_text(json.dumps(manifest, indent=1))
        self._event("collected", **manifest)
        return out, manifest

    async def service_exec(self, command: str, *args: Any, **kwargs: Any):  # type: ignore[no-untyped-def,override]
        words = shlex.split(command)
        if not words or words[0] != SENTINEL or self.vm_spec is None:
            return await super().service_exec(command, *args, **kwargs)
        out, manifest = await asyncio.to_thread(self._collect, words[1:])
        try:
            await super().upload_dir(out, EXPORT)
        finally:
            shutil.rmtree(out, ignore_errors=True)
        from harbor.environments.base import ExecResult  # noqa: PLC0415

        return ExecResult(stdout=json.dumps(manifest), stderr="", return_code=0)

    def _delete_vm(self) -> None:
        if self.vm_spec is None:
            return
        if self.vm is not None:
            try:
                self.vm.delete()
            finally:
                self._event("deleted")
        else:
            subprocess.run([V.TART, "delete", self.vm_name], capture_output=True, check=False)

    async def stop(self, delete: bool) -> None:
        try:
            await super().stop(delete)
        finally:
            await asyncio.to_thread(self._delete_vm)
