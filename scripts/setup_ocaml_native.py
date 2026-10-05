#!/usr/bin/env python3
"""Provision the pinned OCaml library toolchain in a dedicated local opam root."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shlex
import shutil
import subprocess
import sys
import tarfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REVISION = "f294d7f729ac7f68f786bfa2cf2bb0811d9e6e95"
DEFAULT_ROOT = Path.home() / ".local/share/vulcanbench/ocaml-v1-native"


def provision(destination: Path) -> None:
    destination = destination.resolve()
    for tool in ("opam", "make", "cc", "pkg-config"):
        if not shutil.which(tool):
            raise RuntimeError(
                f"Missing prerequisite {tool}; on macOS install opam and pkg-config."
            )
    destination.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "OPAMROOT": str(destination / "opam"), "OPAMYES": "1", "OPAMJOBS": "2"}

    def run(*arguments: str) -> None:
        print("Running:", shlex.join(arguments), flush=True)
        subprocess.run(arguments, env=env, check=True)

    repository = destination / "opam-repository"
    if not (repository / "repo").is_file():
        archive = destination / "opam-repository.tar.gz"
        urllib.request.urlretrieve(
            f"https://github.com/ocaml/opam-repository/archive/{REVISION}.tar.gz", archive
        )
        unpack = destination / "unpack-repository"
        unpack.mkdir(exist_ok=True)
        with tarfile.open(archive) as source:
            source.extractall(unpack, filter="data")
        shutil.move(str(unpack / f"opam-repository-{REVISION}"), repository)
        unpack.rmdir()
    if not (destination / "opam/config").is_file():
        run(
            "opam",
            "init",
            "--bare",
            "--no-setup",
            "--disable-sandboxing",
            "default",
            str(repository),
        )
    switch = destination / "opam/5.2.1"
    if not (switch / ".opam-switch/switch-state").is_file():
        run("opam", "switch", "create", "5.2.1", "ocaml-base-compiler.5.2.1")
    packages = []
    expected = {}
    for line in (ROOT / "tasks/ocaml-v1/TOOLCHAIN.lock").read_text().splitlines():
        name, version = line.split()
        expected[name] = version
        if version != "base":
            packages.append(f"{name}.{version}")
    run("opam", "install", "--switch=5.2.1", *packages)
    listing = subprocess.check_output(
        ["opam", "list", "--switch=5.2.1", "--installed", "--columns=name,version", "--short"],
        env=env,
        text=True,
    )
    actual = dict(line.split() for line in listing.splitlines() if line.strip())
    mismatches = {
        name: actual.get(name) for name, version in expected.items() if actual.get(name) != version
    }
    if mismatches:
        raise RuntimeError(f"Pinned package versions do not match: {mismatches}")
    (destination / "TOOLCHAIN.lock").write_text(listing)
    variables = {
        "OPAMROOT": str(destination / "opam"),
        "OPAMSWITCH": "5.2.1",
        "OCAMLPATH": str(switch / "lib"),
        "DUNE_CACHE": "disabled",
        "VULCANBENCH_COMPILER_SEED": str(destination / "compiler-seed"),
        "VULCANBENCH_COMPILER_SEED_HELPER": str(ROOT / "scripts/ocaml_compiler_seed.py"),
    }
    exports = [f"export {name}={shlex.quote(value)}" for name, value in variables.items()]
    exports.append(
        f'export PATH={shlex.quote(str(ROOT / ".venv/bin"))}:{shlex.quote(str(switch / "bin"))}:"$PATH"'
    )
    (destination / "env.sh").write_text("\n".join(exports) + "\n")
    receipt = {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "opam_repository": REVISION,
        "packages": actual,
        "package_versions_match_linux_lock": True,
        "compiler_seed_ready": (destination / "compiler-seed/artifacts.tar.gz").is_file(),
    }
    (destination / "PROVISIONING.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"Pinned library toolchain ready. Source {destination / 'env.sh'}", flush=True)


def provision_compiler_seed(destination: Path) -> None:
    """Build public pre-fix artifacts for this OS and architecture, without gold."""
    destination = destination.resolve()
    source = destination / "compiler-base"
    seed = destination / "compiler-seed"
    receipt_path = seed / "BUILD.json"
    snapshot = ROOT / "tasks/ocaml-v1/compiler-gadt-field-safety/repo_snapshot.tar.gz"
    snapshot_sha = hashlib.sha256(snapshot.read_bytes()).hexdigest()
    if receipt_path.is_file():
        receipt = json.loads(receipt_path.read_text())
        if (
            receipt["snapshot_sha256"] != snapshot_sha
            or receipt["machine"] != platform.machine()
            or receipt["system"] != platform.system()
        ):
            raise RuntimeError("Existing compiler seed does not match source or native platform")
        if (
            hashlib.sha256((seed / "artifacts.tar.gz").read_bytes()).hexdigest()
            != receipt["artifact_sha256"]
        ):
            raise RuntimeError("Existing compiler seed digest does not match its receipt")
        print(f"Matching native compiler seed already ready: {seed}", flush=True)
        return
    if not (seed / "source-inventory.json").is_file():
        if source.exists():
            raise RuntimeError(f"Uninventoried compiler source already exists: {source}")
        source.mkdir(parents=True)
        with tarfile.open(snapshot) as archive:
            archive.extractall(source, filter="data")
        subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts/ocaml_compiler_seed.py"),
                "inventory",
                str(source),
                str(seed / "source-inventory.json"),
            ],
            check=True,
        )
    started = time.monotonic()
    commands = [
        ["./configure", f"--prefix={destination / 'compiler-install'}"],
        ["make", "-j1", "world.opt"],
    ]
    for command in commands:
        print("Running:", shlex.join(command), flush=True)
        subprocess.run(command, cwd=source, check=True)
    subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts/ocaml_compiler_seed.py"),
            "package",
            str(source),
            str(seed),
        ],
        check=True,
    )
    receipt = {
        "snapshot_sha256": snapshot_sha,
        "source_revision": "364874344779d2e410e99eb634bea8e7e2b70159",
        "system": platform.system(),
        "machine": platform.machine(),
        "platform": platform.platform(),
        "commands": commands,
        "duration_s": time.monotonic() - started,
        "artifact_sha256": hashlib.sha256((seed / "artifacts.tar.gz").read_bytes()).hexdigest(),
        "artifact_count": len(json.loads((seed / "artifact-inventory.json").read_text())),
        "reference_patch_included": False,
        "hidden_tests_included": False,
    }
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"Native public baseline seed ready: {seed}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--compiler-seed-only", action="store_true")
    args = parser.parse_args()
    if args.compiler_seed_only:
        provision_compiler_seed(args.root)
    else:
        provision(args.root)


if __name__ == "__main__":
    main()
