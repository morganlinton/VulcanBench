"""A warmed compiler may supply artifacts, but must never overwrite sources."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tarfile
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/ocaml_compiler_seed.py"
SPEC = importlib.util.spec_from_file_location("ocaml_compiler_seed", SCRIPT)
assert SPEC and SPEC.loader
seed_tool = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(seed_tool)


def prepared_seed(tmp_path: Path) -> tuple[Path, Path]:
    source, seed = tmp_path / "source", tmp_path / "seed"
    source.mkdir()
    (source / "matching.ml").write_text("public baseline\n")
    seed_tool.inventory(source, seed / "source-inventory.json")
    (source / "matching.ml").write_text("changed during build\n")
    (source / "compiler.opt").write_bytes(b"generated executable")
    (source / "compiler.opt").chmod(0o755)
    (source / "compiler").symlink_to("compiler.opt")
    seed_tool.package(source, seed)
    return source, seed


def test_generated_seed_preserves_patched_sources_and_later_artifacts(tmp_path: Path) -> None:
    _, seed = prepared_seed(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "matching.ml").write_text("model patch\n")
    seed_tool.install(seed, workspace)
    assert (workspace / "matching.ml").read_text() == "model patch\n"
    assert (workspace / "compiler").read_bytes() == b"generated executable"
    assert (workspace / "compiler.opt").stat().st_mode & 0o111
    (workspace / "compiler.opt").write_bytes(b"rebuilt from model patch")
    seed_tool.install(seed, workspace)
    assert (workspace / "compiler").read_bytes() == b"rebuilt from model patch"
    assert "matching.ml" not in json.loads((seed / "artifact-inventory.json").read_text())


def test_seed_refuses_existing_artifact_before_any_extraction(tmp_path: Path) -> None:
    _, seed = prepared_seed(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "compiler.opt").write_bytes(b"must survive")
    with pytest.raises(ValueError, match="refusing to overwrite"):
        seed_tool.install(seed, workspace)
    assert (workspace / "compiler.opt").read_bytes() == b"must survive"
    assert not (workspace / "compiler").is_symlink()


@pytest.mark.parametrize("name", ["matching.ml", "../escape", "/absolute"])
def test_seed_rejects_original_source_and_escaping_paths(tmp_path: Path, name: str) -> None:
    _, seed = prepared_seed(tmp_path)
    with tarfile.open(seed / "artifacts.tar.gz", "w:gz") as archive:
        archive.addfile(tarfile.TarInfo(name))
    (seed / "artifact-inventory.json").write_text(json.dumps([name]))
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    with pytest.raises(ValueError, match="unsafe or original-source"):
        seed_tool.install(seed, workspace)
    assert not (workspace / ".vulcanbench-compiler-seeded").exists()


def test_seed_rejects_inventory_mismatch(tmp_path: Path) -> None:
    _, seed = prepared_seed(tmp_path)
    (seed / "artifact-inventory.json").write_text("[]")
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    with pytest.raises(ValueError, match="differs from its inventory"):
        seed_tool.install(seed, workspace)


def test_seed_artifacts_do_not_enter_model_patch(tmp_path: Path) -> None:
    _, seed = prepared_seed(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "matching.ml").write_text("baseline\n")
    subprocess.run(["git", "init", "-q"], cwd=workspace, check=True)
    seed_tool.install(seed, workspace)
    result = subprocess.run(
        ["git", "status", "--porcelain"], cwd=workspace, check=True, capture_output=True, text=True
    )
    assert result.stdout.strip() == "?? matching.ml"


@pytest.mark.parametrize("directory", ["hidden", ".git"])
def test_packager_rejects_private_and_git_additions(tmp_path: Path, directory: str) -> None:
    source, seed = prepared_seed(tmp_path)
    (source / directory).mkdir()
    (source / directory / "secret").write_text("not an artifact")
    with pytest.raises(ValueError, match="unexpected private or Git content"):
        seed_tool.package(source, seed)
