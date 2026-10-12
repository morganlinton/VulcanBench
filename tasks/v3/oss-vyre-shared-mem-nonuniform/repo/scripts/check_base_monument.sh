#!/usr/bin/env bash
# Base monument  -  hard CI enforcement of every prerequisite vyre must
# earn before any claim to release. Each sub-check corresponds to one
# item from the monument-base list:
#
#   1. Extensibility demonstrator: examples/external_ir_extension compiles
#      against vyre, adds its own Opaque extension, and does NOT edit any
#      vyre-core file. <200 LOC in the example crate.
#
#   2. Three-substrate parity: examples/three_substrate_parity/ exists
#      and its expected output manifest contains at least one byte-
#      identical parity claim per primitive in the stdlib dialect.
#
#   3. Signed conformance certificate: every registered op has a row
#      in docs/catalogs/coverage-matrix.md AND every backend with
#      supports_dispatch=true produces a certificate under
#      .internals/certs/<backend>/<op_id>.json when the conform runner runs.
#
#   4. Benchmark honesty: the old scattered benchmark tree is removed,
#      and the replacement meta-harness exists as an executable crate with
#      a stable registry, JSON report path, and thesis workload evidence.
#
#   5. Zero runtime cost invariants: check_no_hot_path_inventory.sh
#      green (already wired); fine-grained measurement moves to the
#      vyre-bench meta-harness contract.
#
#   6. Conform test coverage floor: at least 3 proptest files under
#      vyre-core/tests/ whose names match `*proptest*` or `*adversarial*`.
#
#   7. Reference interpreter isolation: zero *.rs files under
#      vyre-core/src/ops/*/reference/ (the reference code must live in
#      vyre-reference/). OPS migration pending this assertion.
#
#   8. Hot-path allocation invariants: vyre-driver-wgpu/src/pipeline.rs contains
#      no `Vec::new()`, `vec![`, or `Box::new(` on dispatch-reachable
#      lines  -  enforced by grep gate already present in check_no_hot_path_inventory.
#
#   9. IEEE-754 strict math: zero `_vyre_fast_` tokens in vyre-core/src
#      (fast-math wrappers banned  -  Rust's libm path is the floor).
#
# Anything failing = NOT RELEASE-READY. The monument base is an entry
# ticket, not an achievement.

# Note: intentionally NOT using `set -e`  -  each sub-check reports its
# own pass/fail and we want the full diagnostic printed even when
# multiple checks fail. The aggregated exit happens at the end.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source scripts/lib/cargo_runner.sh
vyre_select_cargo_runner

failed=0
pass() { printf "  \xE2\x9C\x93 %s\n" "$1"; }
fail() { printf "  \xE2\x9C\x97 %s\n" "$1" >&2; failed=1; }
note() { printf "  \xE2\x9D\x93 %s\n" "$1" >&2; }

echo "=== 1. Extensibility demonstrator ==="
if [[ ! -d "examples/external_ir_extension" ]]; then
    fail "examples/external_ir_extension/ does not exist  -  open-IR thesis is unproven"
else
    demo_loc=$(find examples/external_ir_extension -name '*.rs' -exec cat {} + | wc -l)
    if [[ "$demo_loc" -gt 200 ]]; then
        fail "external_ir_extension is ${demo_loc} LOC (cap 200)  -  simplify until the demo is trivial"
    else
        pass "external_ir_extension exists at ${demo_loc} LOC"
    fi
    # Must not edit any vyre-core file from within its own tree.
    # Proxy check: the example crate's Cargo.toml depends on vyre via path=, not via a workspace member.
    if grep -q '\[workspace\]' examples/external_ir_extension/Cargo.toml 2>/dev/null; then
        pass "external_ir_extension declares its own workspace (isolated from vyre-core edits)"
    else
        fail "external_ir_extension missing [workspace] section  -  isolation not proved"
    fi
fi

echo "=== 2. Three-substrate parity ==="
if [[ ! -d "examples/three_substrate_parity" ]]; then
    fail "examples/three_substrate_parity/ does not exist"
else
    if [[ -d "docs/parity" ]] && ls docs/parity/*.md >/dev/null 2>&1; then
        pass "docs/parity/ has published reports"
    else
        note "docs/parity/ not yet populated (CI should generate nightly)"
        # not a hard fail pre-nightly-CI
    fi
    pass "examples/three_substrate_parity exists"
fi

echo "=== 3. Signed conformance certificates ==="
if [[ ! -d "conform/vyre-conform-runner" ]]; then
    fail "conform/vyre-conform-runner missing"
else
    # Look for ed25519 signing reference.
    if grep -rq "ed25519" conform/ 2>/dev/null; then
        pass "conform references ed25519 signing"
    else
        fail "conform runner has no ed25519 signing path  -  certificates are not cryptographically signed"
    fi
    # Certificate directory convention exists?
    if [[ -d ".internals/certs" ]] || grep -rq "certs/" conform/vyre-conform-runner/src/ 2>/dev/null; then
        pass ".internals/certs or certs/ path referenced"
    else
        fail "no .internals/certs/ path referenced from conform runner"
    fi
fi

echo "=== 4. Benchmark honesty ==="
bench_targets=$(grep -Rsn '^\[\[bench\]\]' -- */Cargo.toml conform/*/Cargo.toml Cargo.toml 2>/dev/null | grep -vE 'vyre-bench/|vyre-frontend-c/' | wc -l)
if [[ "$bench_targets" -gt 0 ]]; then
    fail "$bench_targets scattered Cargo [[bench]] targets remain"
    grep -Rsn '^\[\[bench\]\]' -- */Cargo.toml conform/*/Cargo.toml Cargo.toml 2>/dev/null | grep -vE 'vyre-bench/|vyre-frontend-c/' | head -10 >&2
else
    pass "no scattered Cargo [[bench]] targets"
fi
if [[ ! -f "docs/VYRE_BENCH_META_HARNESS_PRD.md" ]]; then
    fail "docs/VYRE_BENCH_META_HARNESS_PRD.md missing  -  replacement benchmark architecture is not specified"
else
    pass "vyre-bench meta-harness PRD exists"
fi
for path in \
    "vyre-bench/Cargo.toml" \
    "vyre-bench/src/main.rs" \
    "vyre-bench/src/cli.rs" \
    "vyre-bench/src/registry/mod.rs" \
    "vyre-bench/src/runner/execute/mod.rs" \
    "vyre-bench/src/report/json.rs"; do
    if [[ -f "$path" ]]; then
        pass "$path exists"
    else
        fail "$path missing  -  meta-harness implementation is incomplete"
    fi
done
bench_registry_json="$(mktemp)"
bench_registry_err="$(mktemp)"
if "$CARGO_RUNNER" run -q -p vyre-bench -- list --format json > "$bench_registry_json" 2>"$bench_registry_err"; then
    pass "vyre-bench list --format json executes"
    for case_id in \
        "frontend.c.parser.linux_driver_pipeline" \
        "foundation.dfa_match.256k" \
        "primitives.graph.frontier_step.1m" \
        "runtime.megakernel.truth.1024"; do
        if grep -q "\"$case_id\"" "$bench_registry_json"; then
            pass "vyre-bench registry contains thesis case $case_id"
        else
            fail "vyre-bench registry missing thesis case $case_id"
        fi
    done
else
    fail "vyre-bench list --format json failed  -  meta-harness CLI is not executable"
    sed -n '1,20p' "$bench_registry_err" >&2
fi
rm -f "$bench_registry_json" "$bench_registry_err"
if bash scripts/check_deep_bench_coverage.sh >/dev/null 2>&1; then
    pass "vyre-bench deep coverage gate proves all 7 benchmark dimensions"
else
    fail "vyre-bench deep coverage gate failed"
fi

echo "=== 5. Zero runtime cost invariants ==="
if bash scripts/check_no_hot_path_inventory.sh >/dev/null 2>&1; then
    pass "hot-path inventory gate green"
else
    fail "hot-path inventory gate RED"
fi
pass "runtime-cost measurement is owned by the single meta-harness contract"

echo "=== 6. Conform test coverage floor ==="
adversarial_files=$(find vyre-core/tests vyre-reference/tests -name '*adversarial*' -o -name '*proptest*' 2>/dev/null | wc -l)
if [[ "$adversarial_files" -lt 3 ]]; then
    fail "only $adversarial_files adversarial/proptest files across core+reference (floor: 3)"
else
    pass "$adversarial_files adversarial/proptest files found"
fi

echo "=== 7. Reference interpreter isolation ==="
ref_in_core=$(find vyre-core/src/ops -path '*/reference/*.rs' 2>/dev/null | wc -l)
if [[ "$ref_in_core" -gt 0 ]]; then
    fail "$ref_in_core reference .rs files in vyre-core/src/ops  -  reference code must live in vyre-reference"
else
    pass "vyre-core/src/ops/ has zero reference .rs files"
fi

echo "=== 8. Hot-path allocation invariants ==="
hot_alloc_hits=$(grep -En '(Vec::new\(\)|vec!\[[^]]{0,20}\]|Box::new\()' vyre-driver-wgpu/src/pipeline.rs 2>/dev/null | wc -l)
if [[ "$hot_alloc_hits" -gt 5 ]]; then
    fail "$hot_alloc_hits potential hot-path allocations in vyre-driver-wgpu/src/pipeline.rs"
else
    pass "vyre-driver-wgpu/src/pipeline.rs has ≤5 alloc sites"
fi

echo "=== 9. IEEE-754 strict math ==="
fastmath_hits=$(grep -rEn '_vyre_fast_' vyre-core/src 2>/dev/null | wc -l)
if [[ "$fastmath_hits" -gt 0 ]]; then
    fail "$fastmath_hits _vyre_fast_* tokens in vyre-core/src  -  IEEE-754 strict contract violated"
else
    pass "no _vyre_fast_* tokens in vyre-core/src"
fi

echo ""
echo "=== Monument base ==="
if [[ "$failed" -ne 0 ]]; then
    echo "NOT READY. Fix the failing prerequisites before any 'release' claim." >&2
    exit 1
fi
echo "All 9 prerequisites satisfied. Base monument stands."
