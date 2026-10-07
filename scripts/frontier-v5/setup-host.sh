#!/bin/bash
# Prepare a host (laptop or Claude Code cloud VM) for Frontier v5 authoring.
#
# Builds the four architecture-neutral sandbox tags every v5 task Dockerfile
# starts from, for THIS host's CPU architecture, and installs the pinned
# Harbor CLI. Idempotent: Docker's cache makes re-runs cheap.
#
#   vulcanbench/sandbox:v5-base      Python 3.12 + Go + Node 22 (sandbox/Dockerfile.base)
#   vulcanbench/sandbox:v5-cfamily   + gcc 12, clang 15, sanitizers, cmake, meson (Dockerfile.cfamily)
#   vulcanbench/sandbox:v5-jvm       + Temurin 21, Maven 3.9.16, Gradle 9.8.0 (Dockerfile.jvm)
#   vulcanbench/sandbox:v5-rust      + Rust 1.87.0, clippy, rustfmt, cargo-audit (Dockerfile.rust)
#
# The python:3.12-slim-bookworm base is pinned by an image-manifest digest,
# which is per architecture (docs/frontier-v5/PHASE1.md finding 1), so the
# arm64 digest is passed on Apple silicon and the Dockerfile default (amd64)
# is used everywhere else. The amd64 tags other suites run on
# (vulcanbench/sandbox:base, :jvm, ...) are never touched.
#
# Usage: scripts/frontier-v5/setup-host.sh [--no-rust] [--no-harbor]
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$here"
want_rust=1; want_harbor=1
for a in "$@"; do case "$a" in --no-rust) want_rust=0 ;; --no-harbor) want_harbor=0 ;; *) echo "unknown option $a" >&2; exit 2 ;; esac; done

arch="$(uname -m)"
case "$arch" in
  arm64|aarch64)
    platform=linux/arm64
    # arm64 manifest digest of python:3.12-slim-bookworm, resolved 2026-10-06 (PHASE1.md).
    base_args=(--build-arg PYTHON_IMAGE=python:3.12-slim-bookworm@sha256:739ba32ae445e8d58f3d90feb85f83bebc8346f8dd280fa1eb5848f4ff1ed163) ;;
  x86_64|amd64)
    platform=linux/amd64
    base_args=() ;;
  *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac
echo "frontier-v5 setup: $arch ($platform)"

command -v docker >/dev/null || { echo "docker not found" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "docker daemon not reachable" >&2; exit 1; }

build() { # tag dockerfile [extra build args...]
  local tag=$1 df=$2; shift 2
  echo "== building vulcanbench/sandbox:$tag from $df"
  docker build --platform "$platform" "$@" -t "vulcanbench/sandbox:$tag" -f "$df" .
}
build v5-base    sandbox/Dockerfile.base    "${base_args[@]}"
build v5-cfamily sandbox/Dockerfile.cfamily --build-arg BASE_IMAGE=vulcanbench/sandbox:v5-base
build v5-jvm     sandbox/Dockerfile.jvm     --build-arg BASE_IMAGE=vulcanbench/sandbox:v5-base
if [ "$want_rust" = 1 ]; then
  build v5-rust  sandbox/Dockerfile.rust    --build-arg BASE_IMAGE=vulcanbench/sandbox:v5-base
fi

if [ "$want_harbor" = 1 ]; then
  export PATH="$HOME/.local/bin:$PATH"
  if ! command -v harbor >/dev/null 2>&1; then
    command -v uv >/dev/null || { echo "uv not found; install uv or pass --no-harbor" >&2; exit 1; }
    uv tool install "harbor==0.24.0"
  fi
  echo "harbor: $(harbor --version 2>/dev/null || echo 'installed (version flag unavailable)')"
fi

echo "== images"
docker images --format '{{.Repository}}:{{.Tag}}  {{.Size}}' | grep 'vulcanbench/sandbox:v5-' | sort
echo "frontier-v5 setup: done"
