#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${VYRE_RELEASE_CERT_DIR:-$ROOT_DIR/.internals/certs/release-shards}"
SHARDS="${VYRE_RELEASE_SHARDS:-64}"
BACKEND="${VYRE_RELEASE_BACKEND:-all}"
FEATURES="${VYRE_RELEASE_FEATURES:-gpu}"
WORKERS="${VYRE_CONFORM_PROOF_WORKERS:-16}"
SHARD_WORKERS="${VYRE_RELEASE_SHARD_WORKERS:-4}"
PROFILE="${VYRE_RELEASE_PROFILE:-debug}"

cd "$ROOT_DIR"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

if [[ "$OUT_DIR" != /* ]]; then
    OUT_DIR="$ROOT_DIR/$OUT_DIR"
fi

if ! [[ "$SHARDS" =~ ^[1-9][0-9]*$ ]]; then
    printf 'VYRE_RELEASE_SHARDS must be a positive integer, got %s\n' "$SHARDS" >&2
    exit 2
fi
if ! [[ "$SHARD_WORKERS" =~ ^[1-9][0-9]*$ ]]; then
    printf 'VYRE_RELEASE_SHARD_WORKERS must be a positive integer, got %s\n' "$SHARD_WORKERS" >&2
    exit 2
fi
if [[ "$SHARD_WORKERS" -gt "$SHARDS" ]]; then
    SHARD_WORKERS="$SHARDS"
fi

build_args=(build -p vyre-conform-runner --bin vyre-conform-runner)
if [[ "${VYRE_RELEASE_NO_DEFAULT_FEATURES:-}" == "1" ]]; then
    build_args+=(--no-default-features)
fi
if [[ -n "$FEATURES" ]]; then
    build_args+=(--features "$FEATURES")
fi
case "$PROFILE" in
    debug)
        profile_dir="debug"
        ;;
    release)
        build_args+=(--release)
        profile_dir="release"
        ;;
    *)
        printf 'VYRE_RELEASE_PROFILE must be debug or release, got %s\n' "$PROFILE" >&2
        exit 2
        ;;
esac

CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$CARGO_RUNNER" "${build_args[@]}"

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    target_root="$CARGO_TARGET_DIR"
else
    target_root="$("$CARGO_RUNNER" metadata --no-deps --format-version 1 | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
fi
target_root="${target_root:-$ROOT_DIR/target}"
if [[ "$target_root" != /* ]]; then
    target_root="$ROOT_DIR/$target_root"
fi
RUNNER_BIN="${VYRE_CONFORM_RUNNER_BIN:-$target_root/$profile_dir/vyre-conform-runner}"
if [[ ! -x "$RUNNER_BIN" ]]; then
    printf 'Fix: vyre-conform-runner binary is missing after build: %s\n' "$RUNNER_BIN" >&2
    exit 1
fi

run_shard() {
    local index="$1"
    local shard_path="$2"
    local -a prove_args=(
        prove
        --shard
        "$index/$SHARDS"
        --out
        "$shard_path"
    )
    if [[ "$BACKEND" != "all" ]]; then
        prove_args+=(--backend "$BACKEND")
    fi
    printf 'proving shard %s/%s backend=%s workers=%s -> %s\n' "$index" "$SHARDS" "$BACKEND" "$WORKERS" "$shard_path" >&2
    (
        cd "$ROOT_DIR"
        VYRE_CONFORM_PROOF_WORKERS="$WORKERS" CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$RUNNER_BIN" "${prove_args[@]}"
    )
}

mkdir -p "$OUT_DIR"
shard_paths=()
active_jobs=0
failures=0
for ((index = 0; index < SHARDS; index += 1)); do
    shard_path="$OUT_DIR/shard-${index}-of-${SHARDS}.json"
    shard_paths+=("$shard_path")
    run_shard "$index" "$shard_path" &
    active_jobs=$((active_jobs + 1))
    if [[ "$active_jobs" -ge "$SHARD_WORKERS" ]]; then
        if ! wait -n; then
            failures=$((failures + 1))
        fi
        active_jobs=$((active_jobs - 1))
    fi
done
while [[ "$active_jobs" -gt 0 ]]; do
    if ! wait -n; then
        failures=$((failures + 1))
    fi
    active_jobs=$((active_jobs - 1))
done
if [[ "$failures" -gt 0 ]]; then
    printf 'Fix: %s release conformance shard worker(s) failed.\n' "$failures" >&2
    exit 1
fi

merged="$OUT_DIR/merged.json"
printf 'merging %s shard(s) -> %s\n' "$SHARDS" "$merged" >&2
merge_args=(merge --out "$merged")
merge_args+=("${shard_paths[@]}")
(
    cd "$ROOT_DIR"
    CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" "$RUNNER_BIN" "${merge_args[@]}"
)
printf '%s\n' "$merged"
