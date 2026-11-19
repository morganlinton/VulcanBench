"""Per-task agent images for --agent-container (harness/agent/agent_images.py)."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from harness.agent import agent_images
from harness.agent.agent_images import (
    RECIPES,
    agent_image_tag,
    cli_version,
    ensure_agent_image,
    get_recipe,
    recipe_digest,
)
from harness.agent.providers import ProviderError

BASE_ID = "sha256:" + "ab" * 32
TAG_ID = "sha256:" + "cd" * 32


class FakeDocker:
    """Stands in for the docker CLI: a set of local images and a build log."""

    def __init__(
        self,
        images: dict[str, dict[str, Any]],
        build_rc: int = 0,
        daemon: str = "linux/amd64",
    ) -> None:
        self.images = images
        self.build_rc = build_rc
        self.daemon = daemon
        self.calls: list[tuple[str, ...]] = []
        self.dockerfiles: list[str] = []

    def __call__(  # noqa: PLR0911 - one branch per docker subcommand
        self, *args: str, timeout: float = 60
    ) -> subprocess.CompletedProcess[str]:
        self.calls.append(args)
        if args[0] == "version":
            return subprocess.CompletedProcess(args, 0, f"{self.daemon}\n", "")
        if args[:2] == ("image", "inspect"):
            body = self.images.get(args[-1])
            if body is None:
                return subprocess.CompletedProcess(args, 1, "", "No such image")
            if "--platform" in args:
                wanted = args[args.index("--platform") + 1]
                if wanted not in body.get("_stored_as", ()):
                    return subprocess.CompletedProcess(args, 1, "", "does not provide platform")
                return subprocess.CompletedProcess(args, 0, f"{body['Id']}\n", "")
            return subprocess.CompletedProcess(args, 0, json.dumps(body), "")
        if args[0] == "build":
            self.dockerfiles.append(Path(args[args.index("--file") + 1]).read_text())
            if self.build_rc:
                return subprocess.CompletedProcess(args, 1, "", "step 1\nstep 2\nnpm ERR! 404")
            self.images[args[args.index("--tag") + 1]] = {"Id": TAG_ID, "Architecture": "arm64"}
            return subprocess.CompletedProcess(args, 0, "", "")
        raise AssertionError(f"unexpected docker call {args}")

    @property
    def builds(self) -> list[tuple[str, ...]]:
        return [c for c in self.calls if c[0] == "build"]


@pytest.fixture(autouse=True)
def _no_override(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("VULCANBENCH_AGENT_IMAGE", raising=False)


def _base_images() -> dict[str, dict[str, Any]]:
    return {"vulcanbench/sandbox:rust": {"Id": BASE_ID, "Os": "linux", "Architecture": "amd64"}}


@pytest.mark.parametrize(
    ("text", "release"),
    [
        ("codex-cli 0.166.0", "0.166.0"),
        ("2.1.247 (Claude Code)", "2.1.247"),
        ("grok 1.0.5-alpha.2", "1.0.5-alpha.2"),
        ("0.149.0", "0.149.0"),
    ],
)
def test_cli_version_reads_the_release(text: str, release: str) -> None:
    assert cli_version(text) == release


@pytest.mark.parametrize("text", [None, "", "dev build"])
def test_cli_version_refuses_text_without_a_release(text: str | None) -> None:
    with pytest.raises(ProviderError, match="cannot read a CLI release"):
        cli_version(text)


def test_recipes_cover_the_container_harnesses() -> None:
    assert set(RECIPES) == {"codex", "claude-code"}
    assert get_recipe("claude-code").cli_name == "claude"
    with pytest.raises(ProviderError, match="no image recipe for harness 'cursor'"):
        get_recipe("cursor")


def test_tag_names_harness_release_base_and_recipe(monkeypatch: pytest.MonkeyPatch) -> None:
    tag = agent_image_tag(get_recipe("codex"), "0.166.0", BASE_ID)
    assert tag == f"vulcanbench/agent-codex:0.166.0-b{'ab' * 6}-r{recipe_digest()}"
    # A different Node runtime is a different recipe, so a new tag.
    monkeypatch.setattr(agent_images, "NODE_IMAGE", "node:24-bookworm-slim")
    assert agent_image_tag(get_recipe("codex"), "0.166.0", BASE_ID) != tag


def test_override_skips_docker_entirely(monkeypatch: pytest.MonkeyPatch) -> None:
    docker = FakeDocker({})
    monkeypatch.setattr(agent_images, "_docker", docker)
    monkeypatch.setenv("VULCANBENCH_AGENT_IMAGE", "hand/made:1")
    image = ensure_agent_image("cursor", None, "vulcanbench/sandbox:rust")
    assert image.image == "hand/made:1"
    assert image.override is True
    assert image.provenance()["recipe"] is None
    assert docker.calls == []


def test_missing_task_image_is_a_clear_error(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(agent_images, "_docker", FakeDocker({}))
    with pytest.raises(ProviderError, match="not built locally"):
        ensure_agent_image("codex", "codex-cli 0.166.0", "vulcanbench/sandbox:rust")


def test_cached_image_is_reused_without_a_build(monkeypatch: pytest.MonkeyPatch) -> None:
    images = _base_images()
    images[agent_image_tag(get_recipe("codex"), "0.166.0", BASE_ID)] = {"Id": TAG_ID}
    docker = FakeDocker(images)
    monkeypatch.setattr(agent_images, "_docker", docker)
    image = ensure_agent_image("codex", "codex-cli 0.166.0", "vulcanbench/sandbox:rust")
    assert image.built is False
    assert image.base_id == BASE_ID
    assert docker.builds == []


def test_build_layers_the_cli_onto_the_task_image(monkeypatch: pytest.MonkeyPatch) -> None:
    docker = FakeDocker(_base_images())
    monkeypatch.setattr(agent_images, "_docker", docker)
    image = ensure_agent_image("claude-code", "2.1.247 (Claude Code)", "vulcanbench/sandbox:rust")

    assert image.built is True
    assert image.cli_version == "2.1.247"
    assert image.image.startswith("vulcanbench/agent-claude-code:2.1.247-b")
    (build,) = docker.builds
    joined = " ".join(build)
    # Built for the task image's platform, not the host's.
    assert "--platform linux/amd64" in joined
    assert "CONTENT_PLATFORM=linux/amd64" in joined
    assert "BASE_IMAGE=vulcanbench/sandbox:rust" in joined
    assert "NPM_PACKAGE=@anthropic-ai/claude-code" in joined
    assert "CLI_VERSION=2.1.247" in joined
    assert "CLI_NAME=claude" in joined
    assert f"org.vulcanbench.agent.base-id={BASE_ID}" in joined
    # The layer brings its own Node and never installs into the task's.
    (dockerfile,) = docker.dockerfiles
    assert "/opt/vb-agent/node/bin/node" in dockerfile
    assert "npm install -g" not in dockerfile
    assert 'grep -F "${CLI_VERSION}"' in dockerfile

    provenance = image.provenance()
    assert provenance["base_image"] == "vulcanbench/sandbox:rust"
    assert provenance["node_image"] == agent_images.NODE_IMAGE
    assert provenance["built_now"] is True

    # The second run finds the cached tag.
    again = ensure_agent_image("claude-code", "2.1.247", "vulcanbench/sandbox:rust")
    assert again.image == image.image
    assert again.built is False
    assert len(docker.builds) == 1


def test_build_failure_reports_the_log_tail(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(agent_images, "_docker", FakeDocker(_base_images(), build_rc=1))
    with pytest.raises(ProviderError, match=r"failed:\n(.|\n)*npm ERR! 404"):
        ensure_agent_image("codex", "0.166.0", "vulcanbench/sandbox:rust")


def test_mislabeled_image_builds_for_its_store_platform_and_its_content(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # Containerd store on an arm64 host: the manifest says arm64, the binaries
    # (and the config) are amd64. The build must target arm64 to find the local
    # image, and install an amd64 Node that can run on those binaries.
    images = {
        "vulcanbench/sandbox:rust": {
            "Id": BASE_ID,
            "Os": "linux",
            "Architecture": "amd64",
            "_stored_as": ("linux/arm64",),
        }
    }
    docker = FakeDocker(images, daemon="linux/arm64")
    monkeypatch.setattr(agent_images, "_docker", docker)
    ensure_agent_image("codex", "0.166.0", "vulcanbench/sandbox:rust")
    (build,) = docker.builds
    joined = " ".join(build)
    assert "--platform linux/arm64" in joined
    assert "CONTENT_PLATFORM=linux/amd64" in joined


def test_image_without_the_daemon_platform_builds_for_its_config(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    docker = FakeDocker(_base_images(), daemon="linux/arm64")  # not stored as arm64
    monkeypatch.setattr(agent_images, "_docker", docker)
    ensure_agent_image("codex", "0.166.0", "vulcanbench/sandbox:rust")
    (build,) = docker.builds
    assert "--platform linux/amd64" in " ".join(build)
