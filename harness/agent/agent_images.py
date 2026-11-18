"""Per-task images for agent-in-container runs (``--agent-container``).

A containerized agent must see the same toolchains as the verifier, so its
image is the task's own sandbox image (``resolve_sandbox_image``) with a thin
agent layer on top, not one fixed image for every task. Before this module the
agent always ran in ``vulcanbench/agent-<harness>:latest`` (built on
``sandbox:base``), so a Rust, JVM or C-family task verified with cargo, javac
or cmake while its agent had none of them.

The agent layer is self-contained under ``/opt/vb-agent``: a private Node
runtime, the CLI's npm package and a wrapper on ``PATH`` that runs the CLI
with that Node. Nothing is installed into the task image's own Node or global
``node_modules``, so a JavaScript task keeps exactly the toolchain it pinned.

Images are built on demand and cached by tag. The tag names the harness, the
CLI version, the base image id and a hash of the recipe, so a rebuilt task
image, a CLI bump or a recipe change each produce a new image and an old one
is never silently reused. The CLI version defaults to the host installation's
(the one the preflight reads), keeping container and host runs attributable
to the same release.

Build:  python -m harness.agent.agent_images --harness codex --base vulcanbench/sandbox:rust
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from harness.agent.providers import ProviderError

#: Node runtime the agent layer carries (multi-arch index digest, resolved
#: 2026-11-18 with ``docker buildx imagetools inspect node:22-bookworm-slim``).
#: Its binary needs glibc 2.28+ and libstdc++, which every sandbox image has.
NODE_IMAGE = os.environ.get(
    "VULCANBENCH_AGENT_NODE_IMAGE",
    "node:22-bookworm-slim@sha256:e9f565d32859f0cbfb52125eafe8be39390f24dac302dc3cdf566e8ba3ab21ee",
)

#: Explicit full image for every containerized run, bypassing the per-task
#: build (for an image prepared by hand). Recorded as an override in the trace.
IMAGE_OVERRIDE_ENV = "VULCANBENCH_AGENT_IMAGE"

_BUILD_TIMEOUT_S = 1800


@dataclass(frozen=True)
class AgentImageRecipe:
    """How to install one harness's CLI into the agent layer."""

    harness_id: str
    cli_name: str
    npm_package: str


RECIPES: dict[str, AgentImageRecipe] = {
    "codex": AgentImageRecipe("codex", "codex", "@openai/codex"),
    "claude-code": AgentImageRecipe("claude-code", "claude", "@anthropic-ai/claude-code"),
}

# The agent layer. The CLI stage installs the npm package with the pinned Node
# for the architecture of the task image's binaries (see inspect_image), then writes a wrapper that runs the
# package's bin with that Node (a ``#!/usr/bin/env node`` bin would otherwise
# pick up the task's Node, or fail where the task has none). A bin that is not
# a Node script (a native launcher) is exec'd directly. The final stage copies
# /opt/vb-agent onto the task image and proves the CLI reports the pinned
# version, so a broken layer fails at build time, never mid-run.
_DOCKERFILE = r"""
ARG BASE_IMAGE
ARG NODE_IMAGE
ARG CONTENT_PLATFORM
FROM --platform=${CONTENT_PLATFORM} ${NODE_IMAGE} AS cli
ARG NPM_PACKAGE
ARG CLI_VERSION
ARG CLI_NAME
RUN npm install --no-audit --no-fund --omit=dev --prefix /opt/vb-agent/cli \
        "${NPM_PACKAGE}@${CLI_VERSION}" \
    && mkdir -p /opt/vb-agent/bin /opt/vb-agent/node/bin \
    && cp "$(command -v node)" /opt/vb-agent/node/bin/node \
    && target="$(readlink -f "/opt/vb-agent/cli/node_modules/.bin/${CLI_NAME}")" \
    && if head -c 128 "$target" | head -n 1 | grep -q node; then \
           printf '#!/bin/sh\nexec /opt/vb-agent/node/bin/node %s "$@"\n' "$target"; \
       else \
           printf '#!/bin/sh\nexec %s "$@"\n' "$target"; \
       fi > "/opt/vb-agent/bin/${CLI_NAME}" \
    && chmod 0755 "/opt/vb-agent/bin/${CLI_NAME}" \
    && chmod -R a+rX /opt/vb-agent

FROM ${BASE_IMAGE}
ARG CLI_NAME
ARG CLI_VERSION
COPY --from=cli /opt/vb-agent /opt/vb-agent
USER root
RUN ln -sf "/opt/vb-agent/bin/${CLI_NAME}" "/usr/local/bin/${CLI_NAME}" \
    && "${CLI_NAME}" --version | grep -F "${CLI_VERSION}"
"""


def recipe_digest() -> str:
    """Short hash of everything that shapes the layer besides its build args."""
    return hashlib.sha256(f"{_DOCKERFILE}\n{NODE_IMAGE}".encode()).hexdigest()[:8]


def get_recipe(harness_id: str) -> AgentImageRecipe:
    recipe = RECIPES.get(harness_id)
    if recipe is None:
        raise ProviderError(
            f"agent-in-container mode has no image recipe for harness {harness_id!r}; "
            f"supported: {', '.join(sorted(RECIPES))}"
        )
    return recipe


_SEMVER = re.compile(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?")


def cli_version(version_text: str | None) -> str:
    """The release in a CLI's ``--version`` line (``codex-cli 0.166.0`` -> ``0.166.0``)."""
    match = _SEMVER.search(version_text or "")
    if match is None:
        raise ProviderError(
            f"cannot read a CLI release from {version_text!r}; pass an explicit version"
        )
    return match.group(0)


def _docker(*args: str, timeout: float = 60) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            ["docker", *args],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except FileNotFoundError as exc:
        raise ProviderError("docker is not on PATH; agent-in-container mode needs it") from exc


def _daemon_platform() -> str | None:
    proc = _docker("version", "--format", "{{.Server.Os}}/{{.Server.Arch}}")
    text = proc.stdout.strip()
    return text if proc.returncode == 0 and "/" in text else None


def inspect_image(image: str) -> dict[str, str] | None:
    """``{"id", "platform", "content_platform"}`` for a local image, or None.

    ``platform`` is the one ``docker run`` resolves, which the build must
    target; ``content_platform`` is what the image's binaries actually are
    (its config), which the agent layer's Node and CLI must match. They
    usually agree, but not always: with the containerd image store, a
    ``sandbox:base`` built FROM an amd64 digest on an arm64 host is stored as
    a linux/arm64 manifest holding amd64 binaries. A build pinned to
    linux/amd64 then cannot find the local image, and an arm64 Node copied
    into it has no loader to run with.
    """
    proc = _docker("image", "inspect", "--format", "{{json .}}", image)
    if proc.returncode != 0:
        return None
    body = json.loads(proc.stdout)
    variant = body.get("Variant")
    content = f"{body.get('Os', 'linux')}/{body.get('Architecture', 'amd64')}"
    content = f"{content}/{variant}" if variant else content
    platform = content
    daemon = _daemon_platform()
    if daemon is not None and daemon != content:
        probe = _docker("image", "inspect", "--platform", daemon, "--format", "{{.Id}}", image)
        if probe.returncode == 0 and probe.stdout.strip():
            platform = daemon
    return {"id": str(body["Id"]), "platform": platform, "content_platform": content}


def agent_image_tag(recipe: AgentImageRecipe, version: str, base_id: str) -> str:
    """Cache tag: harness, CLI release, base image id and recipe hash."""
    base_short = base_id.removeprefix("sha256:")[:12]
    return f"vulcanbench/agent-{recipe.harness_id}:{version}-b{base_short}-r{recipe_digest()}"


@dataclass(frozen=True)
class AgentImage:
    """The image a containerized agent runs in, with its provenance."""

    image: str
    base_image: str
    base_id: str | None
    cli_version: str | None
    built: bool
    override: bool = False

    def provenance(self) -> dict[str, object]:
        return {
            "image": self.image,
            "base_image": self.base_image,
            "base_id": self.base_id,
            "cli_version": self.cli_version,
            "node_image": None if self.override else NODE_IMAGE,
            "recipe": None if self.override else recipe_digest(),
            "built_now": self.built,
            "override": self.override,
        }


def ensure_agent_image(harness_id: str, version: str | None, base_image: str) -> AgentImage:
    """Return the agent image for ``base_image``, building it when not cached.

    ``version`` is a CLI release or a raw ``--version`` line (the preflight's),
    parsed only when a build is needed. ``VULCANBENCH_AGENT_IMAGE``
    short-circuits the build. The base image must
    already exist locally (the sandbox images are built by ``make``, never
    pulled), so a missing one is a clear error instead of a registry pull.
    """
    override = os.environ.get(IMAGE_OVERRIDE_ENV)
    if override:
        return AgentImage(override, base_image, None, None, built=False, override=True)

    recipe = get_recipe(harness_id)
    version = cli_version(version)
    base = inspect_image(base_image)
    if base is None:
        raise ProviderError(
            f"task image {base_image!r} is not built locally; build it first "
            "(see the sandbox-image targets in the Makefile)"
        )
    tag = agent_image_tag(recipe, version, base["id"])
    if inspect_image(tag) is not None:
        return AgentImage(tag, base_image, base["id"], version, built=False)

    with tempfile.TemporaryDirectory(prefix="vb-agent-image-") as context:
        dockerfile = Path(context) / "Dockerfile"
        dockerfile.write_text(_DOCKERFILE, encoding="utf-8")
        proc = _docker(
            "build",
            "--platform",
            base["platform"],
            "--tag",
            tag,
            "--build-arg",
            f"BASE_IMAGE={base_image}",
            "--build-arg",
            f"NODE_IMAGE={NODE_IMAGE}",
            "--build-arg",
            f"CONTENT_PLATFORM={base['content_platform']}",
            "--build-arg",
            f"NPM_PACKAGE={recipe.npm_package}",
            "--build-arg",
            f"CLI_VERSION={version}",
            "--build-arg",
            f"CLI_NAME={recipe.cli_name}",
            "--label",
            f"org.vulcanbench.agent.harness={recipe.harness_id}",
            "--label",
            f"org.vulcanbench.agent.cli-version={version}",
            "--label",
            f"org.vulcanbench.agent.base-image={base_image}",
            "--label",
            f"org.vulcanbench.agent.base-id={base['id']}",
            "--label",
            f"org.vulcanbench.agent.recipe={recipe_digest()}",
            "--file",
            str(dockerfile),
            context,
            timeout=_BUILD_TIMEOUT_S,
        )
    if proc.returncode != 0:
        tail = "\n".join((proc.stderr or proc.stdout).strip().splitlines()[-15:])
        raise ProviderError(f"building agent image {tag} on {base_image} failed:\n{tail}")
    return AgentImage(tag, base_image, base["id"], version, built=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build (or find) the containerized-agent image for a task image."
    )
    parser.add_argument("--harness", required=True, choices=sorted(RECIPES))
    parser.add_argument("--base", default="vulcanbench/sandbox:base", help="task sandbox image")
    parser.add_argument(
        "--cli-version",
        default=None,
        help="CLI release to install (default: the host installation's)",
    )
    args = parser.parse_args(argv)

    version = args.cli_version
    if version is None:
        from harness.agent.cli_agents import get_cli_agent_adapter  # noqa: PLC0415 - heavy

        version = get_cli_agent_adapter(args.harness).preflight().version
    result = ensure_agent_image(args.harness, version, args.base)
    print(json.dumps(result.provenance(), indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
