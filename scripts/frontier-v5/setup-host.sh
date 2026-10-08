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
# Egress-proxy hosts (Claude Code cloud VMs): build containers there reach the
# network only through a TLS-intercepting egress gateway, so every fetch in
# the chain fails certificate verification, and plain-HTTP apt is refused.
# When interception CAs are found (--ca DIR_OR_FILE, or on a cloud VM, marked
# by /root/.ccr, the host's locally added CAs in /usr/local/share/ca-certificates)
# the chain is built on a local trust layer, vulcanbench/sandbox:v5-python-ca:
# the same pinned python digest with those CAs added to the system store, the
# Debian sources switched to HTTPS (same signed packages), and the CA env vars
# pip and Node read. Without CAs nothing changes (laptop builds). The egress
# policy must also allow the hosts the chain downloads from. On such a host
# with HTTPS_PROXY set, builds also run on the host network through that
# session proxy (build-time args only, not stored in the images): it is the
# path that applies the cloud environment's own allowed domains, which the
# default container path did not pick up. See docs/frontier-v5/PHASE1.md,
# "Cloud VM setup 2026-10-07".
#
# Usage: scripts/frontier-v5/setup-host.sh [--no-rust] [--no-harbor] [--ca DIR_OR_FILE | --no-ca]
set -euo pipefail
here="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$here"
want_rust=1; want_harbor=1; ca_file=auto
while [ $# -gt 0 ]; do
  case "$1" in
    --no-rust) want_rust=0 ;;
    --no-harbor) want_harbor=0 ;;
    --ca) ca_file="${2:?--ca needs a file}"; shift ;;
    --no-ca) ca_file="" ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
if [ "$ca_file" = auto ]; then
  ca_file=""; [ -d /root/.ccr ] && [ -d /usr/local/share/ca-certificates ] && ca_file=/usr/local/share/ca-certificates
fi

arch="$(uname -m)"
case "$arch" in
  arm64|aarch64)
    platform=linux/arm64
    # arm64 manifest digest of python:3.12-slim-bookworm, resolved 2026-10-06 (PHASE1.md).
    base_args=(--build-arg PYTHON_IMAGE=python:3.12-slim-bookworm@sha256:739ba32ae445e8d58f3d90feb85f83bebc8346f8dd280fa1eb5848f4ff1ed163) ;;
  x86_64|amd64)
    platform=linux/amd64
    # amd64 manifest digest, the Dockerfile.base default.
    base_args=() ;;
  *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac
echo "frontier-v5 setup: $arch ($platform)"

command -v docker >/dev/null || { echo "docker not found" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "docker daemon not reachable" >&2; exit 1; }

proxy_args=()
if [ -n "$ca_file" ] && [ -n "${HTTPS_PROXY:-}" ]; then
  proxy_args=(--network host
    --build-arg "HTTPS_PROXY=$HTTPS_PROXY" --build-arg "https_proxy=$HTTPS_PROXY"
    --build-arg "NO_PROXY=${NO_PROXY:-}" --build-arg "no_proxy=${NO_PROXY:-}")
  echo "== builds go through the session proxy $HTTPS_PROXY"
fi

build() { # tag dockerfile [extra build args...]
  local tag=$1 df=$2; shift 2
  echo "== building vulcanbench/sandbox:$tag from $df"
  docker build --platform "$platform" ${proxy_args[@]+"${proxy_args[@]}"} "$@" -t "vulcanbench/sandbox:$tag" -f "$df" .
}
if [ -n "$ca_file" ]; then
  # Trust layer for an egress-proxy host (see header). The digest is the one
  # Dockerfile.base would use on this architecture.
  py_image="${base_args[1]:-}"; py_image="${py_image#PYTHON_IMAGE=}"
  [ -n "$py_image" ] || py_image="$(sed -n 's/^ARG PYTHON_IMAGE=//p' sandbox/Dockerfile.base)"
  echo "== egress-proxy CAs from $ca_file: building vulcanbench/sandbox:v5-python-ca on $py_image"
  ctx="$(mktemp -d)"; trap 'rm -rf "$ctx"' EXIT
  mkdir "$ctx/ca"
  if [ -d "$ca_file" ]; then cp "$ca_file"/*.crt "$ctx/ca/"; else cp "$ca_file" "$ctx/ca/"; fi
  # Also one PEM bundle, which Dockerfile.jvm imports into Temurin's cacerts.
  # (a newline after each file: a CA file without a trailing newline would
  # otherwise glue two certificates onto one line).
  for f in "$ctx"/ca/*.crt; do cat "$f"; echo; done > "$ctx/vulcanbench-egress-proxy.pem"
  cat > "$ctx/Dockerfile" <<'DOCKERFILE'
ARG PYTHON_IMAGE
FROM ${PYTHON_IMAGE}
COPY ca/ /usr/local/share/ca-certificates/vulcanbench-egress/
COPY vulcanbench-egress-proxy.pem /usr/local/share/vulcanbench-egress-proxy.pem
RUN update-ca-certificates \
    && sed -i 's#http://deb.debian.org/#https://deb.debian.org/#g' /etc/apt/sources.list.d/debian.sources
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
    REQUESTS_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt \
    PIP_CERT=/etc/ssl/certs/ca-certificates.crt \
    NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-certificates.crt
DOCKERFILE
  docker build --platform "$platform" --build-arg "PYTHON_IMAGE=$py_image" -t vulcanbench/sandbox:v5-python-ca "$ctx"
  base_args=(--build-arg PYTHON_IMAGE=vulcanbench/sandbox:v5-python-ca)
fi

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
