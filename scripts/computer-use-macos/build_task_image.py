#!/usr/bin/env python3
"""Build one macOS task's VM image from the base image.

    build_task_image.py tasks/computer-use-macos-v1/<task> [--base vbmac-base] [--force]

A trial clones this image, so every trial of a task starts from the same disk
and seeding is paid once. Steps:
 1. Clone the base (scripts/computer-use-macos/build_base.py) to
    vbmac-task-<slug> and boot it with the task's environment/seed/
    directory shared read-only (tart run --dir), visible in the guest at
    "/Volumes/My Shared Files/seed".
 2. Freeze the clock first: write environment/clock (date(1) format
    MMDDhhmmCCYY) to /Library/VulcanBench/clock, where the vb-clock boot
    daemon reads it, and set the date now, so everything seeded carries
    timestamps from the task's frozen day.
 3. Seed. Files first, from the host: environment/seed_fs.py (if present)
    runs against the clone's Data volume mounted read-write before the first
    boot, so files land with the guest user's uid and no guest privacy prompt
    is raised (the guest agent touching ~/Downloads asks for permission and
    blocks). Then app data in the guest: environment/seed.sh (if present)
    runs as the logged-in user through the guest agent, with AppleScript for
    the apps the base image granted it Automation of.
 4. Finalize: quit every app, remove passwordless sudo (the agent must not be
    able to become root through Terminal), turn the guest's network service
    off (no task needs the internet; the guest agent uses a virtio socket),
    shut down through System Events.
 5. Boot once more to check the clock daemon and the empty network, take the
    start-state screenshot, shut down again, and write a manifest with the
    hashes of everything that went in.

environment/vm.json names the image ({"image": "vbmac-task-<slug>"}); the
build refuses a name that does not match the task directory.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))
from vb_macos import vm as V  # noqa: E402

SHARE = "/Volumes/My Shared Files/seed"
FINALIZE = r"""
set -u
osascript -e 'tell application "System Events" to set procs to name of every process whose background only is false' \
  -e 'repeat with p in procs' -e 'if p is not in {"Finder"} then tell application p to quit' -e 'end repeat' >/dev/null 2>&1 || true
osascript -e 'tell application "Finder" to close every window' >/dev/null 2>&1 || true
svc=$(networksetup -listallnetworkservices 2>/dev/null | sed 1d | grep -v '^\*' | head -1)
[ -n "$svc" ] && sudo -n networksetup -setnetworkserviceenabled "$svc" off
# The base had network while it was built, so Software Update may have found
# an update since; clear its badge now that the network is off for good.
sudo -n defaults delete /Library/Preferences/com.apple.SoftwareUpdate RecommendedUpdates 2>/dev/null
sudo -n defaults write /Library/Preferences/com.apple.SoftwareUpdate LastRecommendedUpdatesAvailable -int 0
sudo -n defaults write /Library/Preferences/com.apple.SoftwareUpdate LastUpdatesAvailable -int 0
defaults write com.apple.systempreferences AttentionPrefBundleIDs -dict
sudo -n softwareupdate --clear-catalog >/dev/null 2>&1
killall Dock 2>/dev/null
sudo -n rm -f /etc/sudoers.d/admin-nopasswd
sudo -n true 2>/dev/null && echo SUDO-STILL-OPEN || echo finalize-ok
"""


def digest(paths: list[Path]) -> str:
    h = hashlib.sha256()
    for p in sorted(paths):
        if p.is_file():
            h.update(str(p.relative_to(REPO)).encode())
            h.update(p.read_bytes())
    return h.hexdigest()[:16]


def main() -> int:  # noqa: PLR0915 - one linear build script
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("task", type=Path)
    ap.add_argument("--base", default="vbmac-base")
    ap.add_argument("--force", action="store_true", help="rebuild even if the image exists")
    ap.add_argument("--out", type=Path, default=Path.home() / "vulcanbench-gate" / "macos-build")
    args = ap.parse_args()
    task = args.task.resolve()
    env = task / "environment"
    spec = json.loads((env / "vm.json").read_text())
    name = spec["image"]
    if name != f"vbmac-task-{task.name}":
        raise SystemExit(f"vm.json image must be vbmac-task-{task.name}, got {name}")
    if V.vm_state(name) is not None and not args.force:
        print(f"{name} exists (use --force to rebuild)")
        return 0
    clock = (env / "clock").read_text().strip()
    if len(clock) != 12 or not clock.isdigit():
        raise SystemExit("environment/clock must be MMDDhhmmCCYY")
    out = args.out / name
    out.mkdir(parents=True, exist_ok=True)
    seed_dir = env / "seed"
    inputs = [env / "seed.sh", env / "seed_fs.py", env / "clock", env / "vm.json"]
    if seed_dir.is_dir():
        inputs += list(seed_dir.rglob("*"))
    t0 = time.time()

    vm = V.VM(name, out)
    vm.clone_from(args.base)
    seed_fs = env / "seed_fs.py"
    if seed_fs.is_file():
        with V.mounted_data(vm.disk_path(), out / "mnt", readonly=False) as root:
            r = subprocess.run(
                [sys.executable, str(seed_fs), str(root)],
                capture_output=True,
                text=True,
                timeout=600,
                check=False,
            )
        (out / "seed_fs.log").write_text(r.stdout + "\n--- stderr ---\n" + r.stderr)
        if r.returncode != 0:
            raise SystemExit(f"seed_fs.py failed ({r.returncode}); see {out / 'seed_fs.log'}")
        print("seeded files from the host", flush=True)
    extra = (f"--dir=seed:{seed_dir}:ro",) if seed_dir.is_dir() else ()
    vm.start(display=spec.get("display", "1280x800"), extra=extra)
    try:
        print(f"desktop after {vm.wait_desktop():.0f} s", flush=True)
        vm.close_client()
        if not vm.agent_ready(180):
            raise SystemExit("guest agent did not answer")
        vm.exec(
            f"echo {clock} | sudo -n tee /Library/VulcanBench/clock >/dev/null && sudo -n date {clock} >/dev/null && date"
        )
        if (env / "seed.sh").is_file():
            seed = (env / "seed.sh").read_text()
            r = vm.exec(f"export VB_SEED={_q(SHARE)}; {seed}", timeout=900, check=False)
            (out / "seed.log").write_text(r.stdout + "\n--- stderr ---\n" + r.stderr)
            if r.returncode != 0:
                raise SystemExit(f"seed.sh failed ({r.returncode}); see {out / 'seed.log'}")
            print("seeded apps in the guest", flush=True)
        r = vm.exec(FINALIZE, timeout=120, check=False)
        if "finalize-ok" not in r.stdout:
            raise SystemExit(f"finalize failed: {r.stdout} {r.stderr}")
        how = vm.shutdown()
        print(f"shutdown: {how}", flush=True)
    except BaseException:
        vm.stop()
        raise

    # Check boot: frozen clock, no network, start screen.
    vm.start(display=spec.get("display", "1280x800"))
    try:
        vm.wait_desktop()
        vm.screenshot(out / "start.png")
        vm.close_client()
        vm.agent_ready(120)
        check = vm.exec(
            "date '+%Y-%m-%d %H:%M'; (curl -s -m 5 -o /dev/null -w '%{http_code}' https://example.com || echo no-network); "
            "sudo -n true 2>/dev/null && echo sudo-open || echo sudo-closed",
            check=False,
        ).stdout.split()
        how2 = vm.shutdown()
    except BaseException:
        vm.stop()
        raise
    manifest = {
        "image": name,
        "base": args.base,
        "inputs_sha256_16": digest(inputs),
        "clock": clock,
        "boot_check": check,
        "shutdown": [how, how2],
        "seconds": round(time.time() - t0),
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1))
    print(json.dumps(manifest), flush=True)
    return 0


def _q(s: str) -> str:
    return "'" + s.replace("'", "'\"'\"'") + "'"


if __name__ == "__main__":
    sys.exit(main())
