#!/usr/bin/env bash
# P1 inventory #86  -  production-file size gate.
#
# Hard rule: production .rs files in workspace crate `src/` trees may not
# exceed MAX_LINES, with an EXPLICIT exception list of files that the
# audit (items 76-84) already calls out for splitting. Any file not on
# the exception list must come in under MAX_LINES; any file on the
# exception list must not grow beyond a per-file SOFT_CAP. The exception
# list is closed  -  adding a new entry is the same kind of change as
# bumping a high-water ratchet, and reviewers should treat it as a
# regression.
#
# Tests, benches, fuzz targets, generated outputs, and the xtask runner
# are scanned with a looser cap because they are not production hot
# paths.
#
# Usage:
#   scripts/check_max_file_size.sh                # enforce
#   scripts/check_max_file_size.sh --report       # list every src/*.rs file with size

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Hard cap for new production files outside the core crates.
MAX_LINES=3000
# Core crates are the substrate contract. Any new production source file above
# this size is a split-required god file; existing oversized files are ratcheted
# below in CORE_AUDIT_EXCEPTIONS so they cannot grow silently.
CORE_MAX_LINES=2500
# Tests/benches/xtask cap (looser by design  -  scans, fuzz harnesses, and
# the xtask catalog dump can legitimately be longer).
TEST_MAX_LINES=8000

# Files explicitly tracked by audit items 76-84. Each is allowed up to
# its own per-file ceiling; this freezes the current sizes against
# regression while the splits land. The mapping is the audit number, the
# current LOC at gate-authoring time, and a hard +5% SOFT_CAP so a
# typo-fix is not blocked but a 200-line addition is.
declare -A AUDIT_EXCEPTIONS
AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/parse/vast.rs"]=9100              # #76  -  8692 LOC C grammar
AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/preprocess/expansion.rs"]=3300    # cpp expansion sibling of #76
AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/lower/ast_to_pg_nodes.rs"]=1670   # cpp lowering sibling of #76
AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/lowering/naga_emit/expr.rs"]=1600    # #77  -  1525 LOC naga emit
AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/lib.rs"]=1400                        # #78  -  backend root with audit-driven additions
AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/lowering/naga_emit/mod.rs"]=1280     # #77 sibling  -  tests extracted to mod_tests.rs
AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/lex/lexer.rs"]=1360               # cpp lex sibling of #76
AUDIT_EXCEPTIONS["vyre-foundation/src/optimizer/passes/fusion.rs"]=820      # #79  -  tests extracted to fusion_tests.rs
AUDIT_EXCEPTIONS["vyre-foundation/src/validate/validate.rs"]=920            # #80  -  tests extracted to validate_tests.rs
AUDIT_EXCEPTIONS["vyre-driver-cuda/src/backend.rs"]=1170                    # #81
AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/pipeline_disk_cache.rs"]=850         # #82  -  tests extracted to pipeline_disk_cache_tests.rs
AUDIT_EXCEPTIONS["vyre-driver-cuda/src/codegen.rs"]=1160                    # #81
AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/preprocess/mod.rs"]=1030          # cpp preprocess sibling of #76
AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/pipeline.rs"]=1000                   # #78 sibling  -  pipeline cache
AUDIT_EXCEPTIONS["vyre-driver/src/pipeline.rs"]=1000                        # registry pipeline
AUDIT_EXCEPTIONS["vyre-runtime/src/megakernel/telemetry.rs"]=820            # #84  -  tests extracted to telemetry_tests.rs
AUDIT_EXCEPTIONS["vyre-reference/src/workgroup.rs"]=900                     # cpu reference workgroup
AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/parse/structure.rs"]=890          # cpp parser sibling of #76
AUDIT_EXCEPTIONS["vyre-foundation/src/optimizer/scheduler.rs"]=1080         # optimizer scheduler  -  tests extracted to scheduler_tests.rs
AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/buffer/handle.rs"]=870               # #78 sibling  -  buffer handle
AUDIT_EXCEPTIONS["vyre-runtime/src/megakernel/protocol.rs"]=780             # protocol enum
AUDIT_EXCEPTIONS["vyre-libs/src/matching/nfa.rs"]=770                       # nfa builder
AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/engine/record_and_readback.rs"]=900  # #78 sibling  -  record path (item #14 lazy trap sidecar)
AUDIT_EXCEPTIONS["vyre-reference/src/hashmap_interp/step.rs"]=760           # interpreter step
AUDIT_EXCEPTIONS["vyre-foundation/src/transform/visit.rs"]=830              # transform visit
AUDIT_EXCEPTIONS["vyre-reference/src/eval_expr.rs"]=840                     # reference eval
AUDIT_EXCEPTIONS["xtask/src/lego_audit.rs"]=870                             # xtask audit dumper

declare -A CORE_AUDIT_EXCEPTIONS
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/parse/vast.rs"]=8692
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/preprocess/expansion.rs"]=3187
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/lower/ast_to_pg_nodes.rs"]=1587
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/lowering/naga_emit/expr.rs"]=1463
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/lex/lexer.rs"]=1292
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/lowering/naga_emit/mod.rs"]=1269
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/optimizer/scheduler.rs"]=1041
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/nn/linear/linear.rs"]=1016
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/preprocess/mod.rs"]=981
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/backend_impl.rs"]=1360
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/pipeline.rs"]=900
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/validate/validate.rs"]=993
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/pipeline_cache.rs"]=882
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/parse/structure.rs"]=844
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/buffer/handle.rs"]=1180
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/engine/record_and_readback.rs"]=829
CORE_AUDIT_EXCEPTIONS["vyre-reference/src/workgroup.rs"]=810
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/engine/multi_gpu.rs"]=808
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/optimizer/passes/fusion.rs"]=804
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/optimizer/fact_substrate.rs"]=570
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/tenant.rs"]=1060
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/megakernel/telemetry.rs"]=791
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/transform/visit.rs"]=789
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/matching/nfa.rs"]=754
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/optimizer/rewrite.rs"]=754
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/megakernel/protocol.rs"]=737
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/buffer/pool.rs"]=910
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/lowering/naga_emit/node.rs"]=709
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/uring/stream.rs"]=830
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/ir_inner/model/program/meta.rs"]=940
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/pipeline_disk_cache.rs"]=677
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/sema/registry.rs"]=660
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/megakernel/io.rs"]=653
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/lower/semantic_edges.rs"]=650
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/validate/expr_rules.rs"]=646
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/ir_inner/model/program/buffer_decl.rs"]=725
CORE_AUDIT_EXCEPTIONS["vyre-reference/src/typed_ops.rs"]=618
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/runtime/tuner.rs"]=618
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/serial/wire/decode/from_wire.rs"]=606
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/transform/autodiff/grad.rs"]=702
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/execution_plan/fusion.rs"]=593
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/megakernel/builder.rs"]=592
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/nn/attention/softmax.rs"]=592
CORE_AUDIT_EXCEPTIONS["vyre-reference/src/eval_expr.rs"]=588
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/math/linalg/matmul_tiled.rs"]=641
CORE_AUDIT_EXCEPTIONS["vyre-primitives/src/math/sinkhorn_iterate.rs"]=740
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/dataflow/ifds_gpu.rs"]=583
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/python/parse/structure.rs"]=700
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/matching/regex_compile.rs"]=579
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/validate/typecheck.rs"]=578
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/math/linalg/matmul.rs"]=815
CORE_AUDIT_EXCEPTIONS["vyre-driver-wgpu/src/runtime/readback_ring.rs"]=720
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/decode/inflate.rs"]=554
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/serial/wire/encode/to_wire.rs"]=690
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/python/lex.rs"]=660
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/validate/nodes.rs"]=653
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/replay.rs"]=549
CORE_AUDIT_EXCEPTIONS["vyre-primitives/src/matching/region.rs"]=544
CORE_AUDIT_EXCEPTIONS["vyre-primitives/src/matching/dfa_compile.rs"]=640
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/go/parse/structure.rs"]=539
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/ir_inner/model/expr.rs"]=539
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/optimizer.rs"]=970
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/optimizer/passes/dead_buffer_elim.rs"]=581
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/execution_plan/mod.rs"]=740
CORE_AUDIT_EXCEPTIONS["vyre-runtime/src/uring/ring.rs"]=685
CORE_AUDIT_EXCEPTIONS["vyre-primitives/src/text/utf8_validate.rs"]=516
CORE_AUDIT_EXCEPTIONS["vyre-reference/src/hashmap_interp/node_step.rs"]=514
CORE_AUDIT_EXCEPTIONS["vyre-foundation/src/execution_plan/policy.rs"]=660
CORE_AUDIT_EXCEPTIONS["vyre-libs/src/parsing/c/sema/lookup.rs"]=507
CORE_AUDIT_EXCEPTIONS["vyre-primitives/src/math/semiring_gemm.rs"]=535

is_test_path() {
    case "$1" in
        */tests/*|*/benches/*|*/fuzz/*|*tests.rs|*xtask/src/*|conform/*) return 0;;
        *) return 1;;
    esac
}

is_core_path() {
    case "$1" in
        # Megakernel/runtime layout is owned by the active runtime restructure;
        # keep this hygiene ratchet from fighting that split while it is moving.
        vyre-runtime/src/megakernel/*) return 1;;
        vyre-foundation/src/*|vyre-runtime/src/*|vyre-reference/src/*|vyre-driver-wgpu/src/*|vyre-libs/src/*|vyre-primitives/src/*) return 0;;
        *) return 1;;
    esac
}

mode="${1:-enforce}"

if [[ "$mode" == "--report" ]]; then
    find . -type f -name '*.rs' \
        -not -path '*/target/*' \
        -not -path '*/target-*/*' \
        -not -path '*/.git/*' \
        -path '*/src/*' \
        -printf '%p\n' | while read -r f; do
        rel="${f#./}"
        lc=$(wc -l < "$f" | tr -d ' ')
        echo "$lc $rel"
    done | sort -rn
    exit 0
fi

violations=()

while IFS= read -r f; do
    rel="${f#./}"
    lc=$(wc -l < "$f" | tr -d ' ')
    if is_test_path "$rel"; then
        cap=$TEST_MAX_LINES
    elif is_core_path "$rel"; then
        if [[ -n "${CORE_AUDIT_EXCEPTIONS[$rel]:-}" ]]; then
            base_cap=${CORE_AUDIT_EXCEPTIONS[$rel]}
            cap=$((base_cap + (base_cap + 19) / 20))
        else
            cap=$CORE_MAX_LINES
        fi
    elif [[ -n "${AUDIT_EXCEPTIONS[$rel]:-}" ]]; then
        base_cap=${AUDIT_EXCEPTIONS[$rel]}
        cap=$((base_cap + (base_cap + 19) / 20))
    else
        cap=$MAX_LINES
    fi
    if (( lc > cap )); then
        violations+=("$rel: $lc lines (cap=$cap)")
    fi
done < <(find . -type f -name '*.rs' \
    -not -path '*/target/*' \
    -not -path '*/target-*/*' \
    -not -path '*/.git/*' \
    -path '*/src/*' \
    -printf '%p\n')

if (( ${#violations[@]} > 0 )); then
    echo "max-file-size gate: ${#violations[@]} violations." >&2
    for v in "${violations[@]}"; do
        echo "  $v" >&2
    done
    echo >&2
    echo "Fix: split the file into focused modules. If the size is structural" >&2
    echo "(grammar table, generated catalog, etc.), bump the per-file cap in" >&2
    echo "scripts/check_max_file_size.sh with explicit rationale and an audit" >&2
    echo "item that tracks the eventual split." >&2
    exit 1
fi

echo "max-file-size gate: every production .rs file is within its cap."
exit 0
