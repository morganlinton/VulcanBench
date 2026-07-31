#!/usr/bin/env bash
set -e
mkdir -p certs
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner
"$CARGO_RUNNER" run -p vyre-conform-runner -- run --backend wgpu --ops all > certs/wgpu_certs.json
echo "Generated certificates in certs/wgpu_certs.json"
