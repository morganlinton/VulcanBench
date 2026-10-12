# Changelog

All notable changes to vyre are documented here. Follows Keep a Changelog.

## [Unreleased]

- Added `GpuLiteralSet::prepare_resident_presence` and the `ResidentPresencePipeline` it returns: a resident literal-set region-presence session that uploads the immutable DFA transition/output/pattern-length tables and suffix-prefilter masks into backend resources ONCE, then re-dispatches across a corpus's coalesced batches re-uploading only the per-file haystack and resetting the per-region presence buffer — eliminating the multi-MiB per-scan table re-upload the borrowed `scan_presence_by_region` path repeats on every file. All-resident so it runs on the CUDA backend.

- Added `ResidentPresencePipeline::scan_into_timed` returning `TimedDispatchResult` (wall / device / enqueue / wait nanoseconds) so callers can attribute a region-presence dispatch's GPU-kernel time separately from host staging and decode; `scan_into` now wraps it. Direct CUDA attribution on an RTX 5090 (8 MiB, 900 detectors) measured the region-presence kernel at ~41 µs — the borrowed path's cost is per-scan table re-upload, not the kernel.

- Made `prepare_resident_presence` fail closed at prepare time when the requested resident haystack capacity is smaller than the NFA program's statically-declared input buffer (binding 0), with an error naming the required byte count and the fix, instead of dispatching against an undersized resident buffer.

- Added `GpuLiteralSet::scan_presence_and_positions_by_region[_with_scratch]`, a single suffix3 dispatch that folds per-region literal presence and confirmed match positions into one GPU pass (previously two separate dispatches), with GPU-vs-exhaustive-CPU-reference differential coverage.

- Added row-strided queue-to-queue delta enqueue for skewed CSR fixpoint waves, wired IFDS queue closure to select it for high-degree rows, and refreshed public API snapshots for the exposed graph/frontier planning surfaces.

- Made the CUDA-resident C sparse lexer compact terminal path read back `out_counts` first and then download only the live dense token column ranges, cutting host transfer volume for sparse translation units without breaking the resident GPU chain.

- Sized C sparse-lexer compact outputs from the scanned token count instead of source byte count for staged and block-total compaction paths, reducing readback and downstream token-buffer pressure on whitespace-heavy translation units.

- Made budgeted resident CSR queue batches plan ordered chunks from each chunk's effective frontier popcount, so sparse runs before and after a dense outlier still pack tightly under the resident scratch budget.

- Clamped resident CSR frontier-queue dispatch capacity from in-domain frontier popcount, reducing graph-sized scratch allocation and overlaunch for sparse single-query and batched traversals while keeping caller queue capacity as a hard cap.

- Sized resident adaptive sparse-queue traversal from the active frontier popcount instead of graph node count, reusing larger queue scratch across smaller frontiers and preserving row-strided traversal for high-degree rows.

- Added 30,000 generated row-strided CSR queue primitive checks covering skewed graph traversal, caller-owned output reuse, malformed CSR rejection, and dispatch-grid coverage.

- Routed the IFDS skewed active-queue and queue-materialization benchmarks through the row-strided CSR queue consumer for high-degree rows, increased the benchmark fixture hub degree to 2,048 edges, and added telemetry proving when the strided traversal path is active.

- Added a row-strided CSR queue traversal primitive for skewed active frontiers, wired resident CSR and adaptive sparse-queue paths to select it for high-degree rows, and refreshed the `vyre-primitives` public API snapshot.

- Made the sparse C tokenizer's raw `U8` haystack runtime-sized, removing the host-side bucket padding copy before token classification while keeping bucketed GPU output shapes.

- Moved the full C comment/splice fallback to runtime-sized raw `U8` source buffers, removing the remaining padded splice-input staging from the byte-filter pipeline.

- Added a backend-extension gate proving new backends remain one crate plus `inventory::submit!`, and declared SPIR-V dispatch capability through the same inventory path as CUDA and wgpu.

- Hardened the base monument benchmark check so it proves the executable `vyre-bench` meta-harness, JSON registry, thesis workload IDs, and deep coverage dimensions instead of only checking for the PRD.

- Added a million-node graph frontier benchmark to `vyre-bench`, with exact CPU-oracle verification and release-suite thesis coverage contracts so benchmark evidence cannot regress to element-wise-only workloads.

- Added explicit graph launch sizing for CSR frontier degree-sum and refreshed the public API snapshot for the current graph/dispatch surfaces.

- Added explicit RLE segment-length dispatch sizing and multi-block CPU/CUDA parity coverage for packed decode workloads.

- Added explicit bigint add-carry dispatch sizing and multi-block CPU/CUDA carry-pattern coverage for large limb arrays.

- Added explicit union-find dispatch sizing through the self-substrate path and multi-block CUDA coverage for large edge batches.

- Added explicit d-DNNF evaluation dispatch sizing and multi-block CUDA coverage for literal-heavy knowledge-compile waves.

- Reworked Scallop single and wide lineage fixpoint kernels to preserve high-cell and high-word seed facts without CUDA grid-barrier races, with CUDA parity coverage for the exposed high-word case.

- Restored multi-block Scallop dispatch for large relation matrices through split-visible GridSync phases while keeping small matrices on the block-local persistent path.

- Packed `tensor_flow_forward` source-node dataflow lanes into 256-lane workgroups and added CUDA parity for context/field propagation past the first block.

- Made GPU region dedup cluster-aware for nested/touching scanner spans, added merged-end metadata for on-device compaction, and proved multi-workgroup CUDA parity.

- Added a 256-lane parallel `bracket_match` path when parser depth caps cannot affect output, with CUDA parity for large nested token streams and retained bounded-stack fallback for overflow-capped shards.

- Routed large adaptive sparse-queue traversal frontiers through the deterministic word-prefix queue materializer, with resident CUDA parity for a large sparse graph step and refreshed adaptive traversal program-cache identities.

- Replaced multi-block word-prefix queue scatter's per-word previous-block loop with an in-place block-offset scan and precomputed-offset scatter, with resident CSR/adaptive wiring and live CUDA coverage for generated multi-block frontier queries.

- Added a CSR-only resident adaptive sparse-queue graph upload and step path so sparse-queue workloads avoid dense adjacency allocation/upload, with live CUDA telemetry coverage and generated sparse-queue matrix coverage on the no-dense path.

- Added CSR frontier queue property gates covering 40,000 generated materialization, traversal, adversarial queue, and validation cases, and doubled live CUDA adaptive sparse-queue generated coverage to 1,024 resident steps per materializer.

- Removed the redundant resident atomic sparse-queue `queue_len` init dispatch from CSR and adaptive traversal paths, dropping small resident sparse-queue steps from four kernels to three while keeping queue length initialization inside `frontier_to_queue`.

- Added packed-`U8` line indexing, UTF-8 validation, and C line-splice classification for text scans, fixed CUDA/PTX byte and halfword memory ops, and covered the paths with generated reference parity plus live CUDA boundary matrices.

- Moved the C preprocessing byte filter to raw `U8` source buffers through preflight, line/block comment paths, full comment masking, and compact scatter, fixed literal-close handling before later comments, and added live CUDA generated-corpus coverage for the end-to-end filter.

- Moved the sparse C tokenizer pipeline to a raw `U8` haystack while preserving packed and expanded compatibility entrypoints, with reference-eval ABI checks and live CUDA generated-corpus parity for token and directive columns.

- Moved the C directive-metadata stage used by the preprocessing pipeline to raw `U8` source bytes while preserving the packed standalone ABI, eliminating another source repack between tokenization and directive classification.

- Moved fused `#define`/`#include`/`#undef` payload parsing in the preprocessing pipeline to raw `U8` source bytes while preserving packed standalone parser ABIs.

- Moved `#ifdef`/`#ifndef` and `#if`/`#elif` compatibility evaluators in directive extraction and live conditional re-evaluation to raw `U8` source rows and macro-name tables while preserving packed standalone evaluator ABIs.

- Removed the now-unused C GPU-preprocess U32 byte-padding staging helper so raw-byte directive and live conditional paths cannot route back through padded host macro-name buffers.

### New

- **`vyre-foundation`  -  effects-handler lowering is on the release path.**
  `PassScheduler` now has an effects-handler enforcement gate: rewrites may
  discharge existing effects, but any newly introduced effect row bit is
  reverted unless the pass declares it through `allowed_effect_additions`.
  Backend `pre_lowering::optimize` enables this gate beside cost-monotone
  enforcement, and pass metrics now expose before/after effect-row bits.
- **`vyre-foundation`  -  linear BufferAccess is on the release path.**
  `PassScheduler` now enforces `BufferDecl::linear_type` postconditions for
  backend pre-lowering: rewrites may repair existing violations but cannot
  introduce new linear/affine/relevant usage violations before lowering. Pass
  metrics expose before/after linear-violation counts.
- **`vyre-foundation`  -  liquid BufferDecl shapes are on the release path.**
  `PassScheduler` now enforces `BufferDecl::shape_predicate` postconditions for
  backend pre-lowering: rewrites may repair existing shape violations but cannot
  introduce new predicate/count contradictions before CUDA or WGPU lowering.
  Pass metrics expose before/after shape-violation counts.
- **`vyre-foundation`  -  liquid shapes now erase dynamic loop guards.**
  `loop_var_range_fold` consumes `ProgramShapeFacts` so comparisons between a
  loop induction variable and `buf_len(buffer)` fold when `ShapePredicate`
  min/max facts prove the branch true or false. Runtime-sized buffers with
  `AtLeast`/`Exactly`/bounded affine shape facts can now drop redundant
  per-iteration bounds checks before CUDA lowering.
- **`vyre-foundation`  -  wire parser adversarial properties run in normal CI.**
  Added generated `Program::to_wire`/`Program::from_wire` property coverage for
  10,000 generated programs, 10,000 arbitrary hostile byte blobs, 10,000
  truncations, and 10,000 digest-refreshed body mutations. The new tests found
  and fixed a decoder gap where tampered but checksum-correct bytes could
  produce zero workgroup dimensions; `from_wire` now rejects zero workgroup
  dimensions and invalid output byte ranges at parse time.
- **`vyre-foundation` / `vyre-driver-cuda` / `vyre-reference`  -  explicit
  single-rank collectives execute through one shared transform.** Added
  substrate-neutral lowering for `CommGroup::WORLD` `AllGather` and
  `ReduceScatter` into bounded copy IR while reducing single-rank `AllReduce`
  and root-0 `Broadcast` to identity semantics. CUDA dispatch, CUDA compiled
  pipelines, and the reference oracle now consume the same transform. Non-world
  groups and nonzero single-rank broadcast roots fail closed with actionable
  errors, so multi-rank transport is never silently emulated. New proptests
  generate 16,384 collective-lowering/reference cases and live CUDA tests cover
  host dispatch, native compiled pipelines, and adversarial root rejection.
  Capability scanning now distinguishes lowerable single-rank collectives from
  collectives that genuinely require transport, and the canonical pre-emit
  pipeline applies the same transform before descriptor lowering.
- **`xtask` / release gates  -  recursion thesis is load-bearing.** Repaired
  `recursion-gate` root detection for the standalone Vyre workspace, made it
  scan the current `vyre-self-substrate/src` tree recursively plus the primitive
  catalog surface, taught it to parse grouped Rust imports across newlines,
  excluded private helper modules from the public primitive inventory, and
  wired `scripts/check_recursion_gate.sh` into release signoff so missing
  self-consumers fail release validation. Added the self-substrate
  `data::parsing_dispatch_pipeline` so packed-AST constant folding and
  bytecode dispatch-table packing consume the parsing primitives on the
  production substrate path.
- **`vyre-foundation`  -  derived pass-order artifact.** Added
  `optimizer::derived_order` with a live inventory-derived pass order,
  declared requirement edges, causal invalidation adjacency, and
  adjustment-set back-door safety checks. Release pass-order validation now
  consumes this artifact instead of reconstructing an independent ordering.
- **`vyre-foundation`  -  planar rewrite batching on the optimizer execution
  path.** Added a foundation-owned non-overlap batch planner,
  `ProgramPass::batch_apply`, refusal-aware `try_batch_apply`, and scheduler
  wiring so high-candidate passes can apply disjoint rewrite waves instead of
  relying on one-candidate-at-a-time launches. The primitive reference oracle
  now delegates to the same planner, keeping CPU contracts and GPU primitive
  tests on one source of truth. The batch activation threshold is runtime
  configurable through `VYRE_PLANAR_REWRITE_BATCH_THRESHOLD`.
- **`vyre-driver` / `vyre-driver-wgpu`  -  natural-gradient launch resolver
  on release paths.** Exported the canonical workgroup candidate table and
  shared launch resolver, wired CUDA `LaunchPlan` and WGPU pre-lowering
  config through safe-gated natural-gradient cold-start workgroup selection,
  and cached the selected launch shape per program/element-count/limit tuple
  so the hot path does not rebuild policy vectors. CUDA timed dispatch now
  records real `device_ns` measurements back into the bounded launch cache,
  allowing later automatic launches to move away from the cold-start
  heuristic when hardware timing proves another candidate faster. WGPU timed
  dispatch now returns timestamp-query `device_ns` as structured
  `TimedDispatchResult` data and feeds it into the same launch-feedback path.
  Measured launch decisions now persist across process restarts through the
  existing bounded tuner TOML cache.
- **`vyre-primitives`  -  dominator-tree public primitive surface.** Added
  the registered graph primitive to the self-consumer catalog, moved its
  scale/VRAM benchmark into the central `vyre-bench` release harness, and
  refreshed the public API snapshot for the new graph contract.
- Document `vyrec` / `vyre-frontend-c` as beta active-development consumers
  rather than the core Vyre `0.4.2` release proof.
- [A06] Document workspace member listing convention (S13)
- [A11] Bulk-fill Jules ticket queue (fixture_sweep + cve_replay)
- [A05] Examples consume published crates via patch.crates-io
- [A03] Validator error code documentation (S8)
- [A02] Rename vyre-cc to vyre-frontend-c
- **`vyre-foundation`  -  `BinOp::MulHigh` IR primitive.** Widening unsigned
  32×32→64 multiply returning the upper 32 bits. Wire tag `0x21`.
  Full support: const-fold in `ir_eval.rs`, interpreter in `node_kind.rs`,
  wire encode/decode in `bin_op_tag.rs`/`bin_op_from_tag.rs`, and
  `Expr::mulhi()` builder. Required for Granlund-Montgomery division.

- **`vyre-foundation`  -  Granlund-Montgomery constant division.** Strength-reduce
  pass now rewrites `x / d` (for constant non-power-of-two `d`) into a
  `MulHigh + Shr` sequence using Hacker's Delight Algorithm D. Eliminates the
  ~70-cycle hardware division in favor of ~5-cycle multiply-shift. Exhaustive
  correctness tests cover all divisors 2–1000 plus extreme boundary cases
  (2³¹±1, 2³²−1). Located in `optimizer/passes/strength_reduce/arithmetic.rs`.

- **`vyre-driver`  -  `LoweringStrategy` trait + capability-driven selector.**
  Two-layer optimization architecture: Layer 1 (IR-level math rewrites in
  `vyre-foundation/optimizer/passes/`) is backend-agnostic. Layer 2 (backend
  lowering strategies in `vyre-driver/strategy/`) is target-dependent.
  Strategies declare capabilities via `BackendCapabilities` and are selected
  by priority. `select_strategy()` picks the highest-priority applicable
  strategy. See `docs/ARCHITECTURE.md § Two-layer optimization architecture`.

- **`vyre-libs`  -  `c_lower_ast_to_pg_nodes` Cat-A op.** Added registration for
  `vyre-libs::parsing::c::lower::ast_to_pg_nodes`, a pure-IR lowering from
  structural VAST rows to packed `PgNode` tuples
  `(kind, span_start, span_end, parent_idx, payload_lo, payload_hi)`.
  Added witness fixture, pure CPU reference oracle, WGSL emission smoke test,
  GPU dispatch parity sample, and adversarial coverage (60 fixtures + proptest).

- **`vyre-runtime`  -  persistent megakernel + `io_uring` NVMe streaming.**
  The GPU becomes a VIR0 bytecode interpreter that loops forever reading
  slots the host publishes into a ring. Linux-only NVMe zero-copy via raw
  `io_uring_setup` + mmap of SQ/CQ rings, with a `uring-cmd-nvme` feature
  for `IORING_OP_URING_CMD` passthrough (kernel 6.0+). Three-buffer
  layout (control / ring / debug_log), 256-lane × N-workgroup sharding,
  opcode extension hook for vendor intrinsics, per-tenant authorization
  masks, atomic `done_count` counter, and a PRINTF debug channel.
- **`vyre-libs`  -  Category A composition ecosystem.** Pure-IR
  compositions over `vyre-ops` primitives (`math`, `nn`, `matching`,
  `crypto`). No raw shader source  -  every library function is a
  `Program` consumers can round-trip, validate, and inline.
  `substring_search` lands with a real byte-by-byte equality instead of
  the earlier LAW 1 placeholder.
- **10 io_uring + IR innovations.** `IORING_REGISTER_BUFFERS` +
  `READ_FIXED`, `IORING_REGISTER_FILES` + `IOSQE_FIXED_FILE`, GPUDirect
  Storage `GpuMappedBuffer::from_bar1_peer`, `futex_waitv` completion
  doorbell, per-workgroup slot sharding, ring-credit backpressure,
  opcode extension hook, tenant-mask routing, PRINTF debug channel,
  AF_XDP/RDMA ingress demonstrated via a TCP smoke test.
- **Error-code catalog grew a `P-*` family** for
  `vyre-runtime::PipelineError`.
- **Workspace docs pristine.** `cargo doc --workspace --all-features
  --no-deps` runs clean  -  zero unresolved intra-doc links, zero
  private-link leakage, zero output collisions.

### Fixed

- **LAW 1 placeholder in `vyre-libs::matching::substring_search`**  -  the
  inner-byte check was `Expr::u32(1)` (matched every position); now
  `load(haystack, i+k) == load(needle, k)` routed through a select to
  stay integer. Gap L-7 closed with a structural regression test that
  fails if the compare ever collapses back to a constant.
- **LAW 9 evasion audit sweep**  -  removed all `// TODO` / `// FIXME`
  markers from shipped code. Subgroup intrinsics return a structured
  error pointing at RFC 0004 instead of a TODO; the autotune workgroup
  heuristic is documented as intentional default instead of a TODO.
- **Driver binary name collision**  -  `vyre-driver-wgpu`'s CLI bin
  renamed from `vyre` → `vyre-wgpu` so it no longer collides with the
  `vyre` lib target in `cargo doc`.
- **Workspace version drift**  -  `vyre-runtime` workspace dep bumped
  from `0.1.0` → `0.6.0` to match the crate's own manifest.
- **`vyre-libs::security::aliases_dataflow` RAW-hazard barrier gap.**
  The local `merge_programs` helper concatenated the seed / hop /
  merge / intersect / union sub-programs without inserting any
  `Node::Barrier`. Threads in later warps observed pre-seed
  `reach_x_buf` state and the BFS frontier silently dropped nodes
  past the warp boundary on every aliases-using rule. Routed
  through `vyre_foundation::execution_plan::fusion::fuse_programs`
  so RAW/WAR hazards get precise barriers. Local helper deleted.
  Two regression tests pin the structural barrier presence and
  unique non-Workgroup binding numbering in the fused output.
- **`vyre-libs::parsing::python` validator-rejected programs.**
  Lex-level `is_ident_start` / `prev_identish` lets stored bool
  exprs that the validator rejected when later compared with
  `u32(0)`; coerced through `select` so the bool→u32 lift happens
  at the let_bind. Structure / call / decorator extractors hoisted
  every cross-block name (`name_end`, `cursor`, `dot_pos`,
  `after_dot`, `target_tok`, `target_name`, `target_kind`,
  `async_def`, `after_decorator`, `after_type_params`, `after_params`,
  `decorator_end`) into the outer body so they outlive the
  if-then blocks that assign them, with new
  `search_next_token_into` / `find_matching_delimiter_into`
  assign-only helpers used inside if-blocks to skip the redundant
  outer let_bind. Closes 13 cascading V008 / V032 / undeclared-var
  validation errors that hid behind a single bool/u32 mismatch.
- **`vyre-primitives::reduce::workgroup_tree`** E0382 use-of-moved-
  value on `dtype: DataType` consumed three times in a single
  `Program::wrapped` BufferDecl block; first two uses now
  `dtype.clone()` so the third use lands on the still-owned value.
- **`vyre-primitives::effects::handler_apply::tests::from_bits_round_trip`**
  literal `0b101_0011` corrected to `0b0010_1011` (bits 0, 1, 3,
  5 = BufferWrite + Atomic + GpuDispatch + AsyncLoad). The pre-fix
  literal had bits 0, 1, 4, 6 set (Atomic + Barrier + Trap) but
  the assertions read GpuDispatch / AsyncLoad → guaranteed test
  failure regardless of the runtime behavior.
- **`vyre-libs::nn::attention::attention_reference_program`** signature
  drift: the function returns `Program` but the body used `?` /
  `Ok(...)`, which only compile under a `Result<…>` return.
  Reverted to panic-on-overflow (callers wanting the fallible path
  go through `try_attention_reference`, which already returns
  `Result<Program, TensorRefError>`).

### Changed

- **Driver boundary and shared-driver lifts.** Concrete backend crates now own
  concrete runtime/API names, while `vyre-driver` hosts shared AOT emitter
  registration, validation cache, binding/program walks, specialization maps,
  tuner framework, subgroup taxonomy, and cross-dispatch fusion decisions.
  Public API snapshots were refreshed for the resulting shared surfaces.
- **Frozen/public API snapshots refreshed.** Snapshots now reflect the
  intentional 0.6 contract surface for borrowed output reuse, borrowed async
  dispatch, subgroup visitors, required lowering implementations, categorical
  laws, and the current published public items for driver/wgpu/foundation/
  primitives/spec crates.
- **`vyre-foundation` program-shape analysis surface.** Public snapshots now
  include `program_shape_facts`, the reusable buffer-shape analysis used by
  optimizer passes and downstream cache consumers.
- **`Node::forever(body)`** helper in `vyre-foundation::ir::Node`. Linus
  principle  -  `forever` lowers to `Node::Loop { 0..u32::MAX, body }`,
  no new enum variant, no cascade of match arms. Persistent kernels
  use it.

## [0.6.0]  -  2026-04-19
(layered workspace: foundation → driver → ops; single inventory registration path)

### New in 0.6.0

- **Nine-crate layered workspace.** Extracted `vyre-foundation` (IR, wire format, visitor traits, extension resolvers), `vyre-driver` (registry, runtime, pipeline, routing, diagnostics), `vyre-driver-wgpu` (wgpu backend, buffer pool, bind-group cache, pre-recorded dispatch), `vyre-driver-spirv`, `vyre-ops` (stdlib dialects), from what was a single god-crate. `vyre` remains as a back-compat meta shim.
- **Machine-checked layer DAG.** `scripts/check_layering.sh` enforces R1–R3+R5 from `COMPUTE_2_0.md §3`: foundation has no driver/ops/backend deps, driver has no ops/backend deps, ops has no backend deps, reference has no backend deps. Cross-layer imports go DOWN only; violations fail CI.
- **True IR openness.** `Expr::Opaque` and `Node::Opaque` now round-trip through the wire format (tag `0x80`) via inventory-registered `OpaqueExprResolver` / `OpaqueNodeResolver`. Validator, optimizer passes, and visitor adapters all honour Opaque explicitly  -  no wildcard fallthrough remains in foundation transforms.
- **Single op registration path.** `inventory::submit!{OpDefRegistration::new(...)}` is THE way to publish an op. `OpSpec` surface is gone; `DialectRegistry` is the frozen index.
- **Zero-alloc dispatch hot path.** `bound_handles` returns `SmallVec<[_; 8]>`, bind groups cache keyed by bound-buffer identity, buffer pool recycles power-of-two allocations across dispatches.
- **`vyre-reference` Memory** replaced `HashMap<String, Buffer>` with `BufferMap` (`SmallVec<[(Arc<str>, Buffer); 8]>`)  -  branch-predicted inner-loop lookups, no per-access SipHash, no per-name `String` allocs. `LocalSlots` interns via `FxHashMap<Arc<str>, _>`.
- **Invariant catalog truthful.** Every descriptor in `vyre-spec/src/invariants.rs` now references a real file at `conform/vyre-conform-enforce/tests/invariants.rs`, enforced by `scripts/check_invariant_paths_exist.sh`.
- **Ratchet CI gates.** `scripts/check_no_string_wgsl.sh` caps Law-B string-WGSL violations at 54 and `naga::front::wgsl::parse_str` sites at 84. `scripts/check_warning_budget.sh` caps workspace warnings at 921. Each gate decreases only; regression fails CI.

### Breaking

- Op registrations must go through `vyre-driver::registry::OpDefRegistration`. Consumers using legacy `OpSpec` surface must migrate.
- `vyre-core/src/` is reduced to `lib.rs` (meta-shim re-exports). Files that reached into `vyre::ir::transform::...` etc. must import from `vyre_foundation` directly  -  the meta-shim still provides the `vyre::ir::X` paths for surgec/pyrograph/warpscan consumers.

## [0.5.0]  -  2026-04-19
(substrate-neutral IR: open extensions + conform certificates)

### New in 0.5.0 final

- **VIR0 wire-format spec published**  -  `vir0-spec.md` at repo root declares the wire format stable across 0.5.x, reserves the `0x80..=0xFF` tag range for third-party extensions in perpetuity, and documents conformance requirements for non-Rust bindings (Phase 22).
- **Bytes extraction validation**  -  `BufferDecl::with_bytes_extraction(true)` opt-in relaxes V013 on load/store of `DataType::Bytes` buffers for legitimate bytes-producing ops like `decode.base64`, `compression.lz4_decompress`, and the decoder family. `Signature` gained `#[non_exhaustive]` + `bytes_extraction` field + `bytes_extractor` constructor (Phase 3).
- **Canonicalized 7 primitive programs** to match the emit-asserted WGSL shape  -  `abs_diff` routes through `max(a,b) - min(a,b)`, `div` / `mod` wrap in zero-guard `select`, `logical_not` uses boolean-style `select(x==0, 1, 0)`, `negate` uses two's-complement `~a + 1`, and `shl` / `shr` zero-guard shifts `>=32` (Phase 2).
- **photonic backend crate** lives in `backends/photonic/` as a registered non-dispatching substrate with `supports_dispatch = false`  -  proves the three-substrate surface claim today, while photonic compute remains future work.
- **SPIR-V backend skeleton** in `backends/spirv/`  -  `SpirvBackend::emit_spv` consumes `naga::Module` built by the shared builder family and calls `naga::back::spv::write_vec`, giving vyre a second real compute-capable backend alongside wgpu (Phase 14).
- **Conform crates scaffolded**  -  `vyre-conform-spec` (witness sets + composition laws), `vyre-conform-generate` (proptest-style shrinking minimizer), `vyre-conform-enforce` (algebraic-law prover over witness pairs), `vyre-conform-runner` (CLI + Certificate schema) at `conform/vyre-conform-*` (Phase 17).
- **rules/op/ certificate library**  -  5 op certs (`decode.base64`, `compression.lz4_decompress`, `match.dfa_scan`, `string_matching.aho_corasick_scan`, `graph.bfs`) plus `SCHEMA.md` defining op_id / signature_blake3 / allowed_backends / witness_set_blake3 / laws metadata (Phase 4).
- **NFA bytecode micro-interpreter fully retired**  -  the remaining `nfa_scan` kernel was deleted in the 2026-04-19 zombie sweep, README/CHANGELOG/VISION cross-references scrubbed, scan and lexical ops now compose in vyre IR end-to-end (Phase 7).
- **Docs**  -  `docs/THESIS.md`, `docs/ARCHITECTURE.md`, `docs/memory-model.md`, `docs/targets.md`, `docs/wire-format.md` authored as load-bearing spec.

### Breaking

- `Signature` is `#[non_exhaustive]`  -  out-of-crate literal construction must move to `Signature::bytes_extractor(...)` or `Signature { inputs, outputs, attrs, ..Signature::default() }` equivalent.
- `BufferDecl` gained the `bytes_extraction: bool` field; source-compatible through the builder API (`::read`, `::output`, `::read_write`, `::storage`, `::workgroup`), but direct struct literals must set it.

### Fixed

- `all_primitives` arithmetic / bitwise assertions now see the canonical WGSL shapes emitted by `naga_emit`  -  `abs_diff`, `div`, `mod`, `logical_not`, `negate`, `shl`, `shr` all validate against the assertion set.
- V013 no longer blocks valid decode / decompress flows that read and write typed `Bytes` buffers.
- README no longer describes a bounded `nfa_scan` bytecode micro-interpreter; it was deleted.

### Substrate (Claude)
- core: structured `Diagnostic` API with stable `E-*` / `W-*` codes,
  rustc-style human render, JSON round-trip for LSP / CI integration
  (A-C1b).
- wire: rev 3 framing  -  schema version bumped to 3 with structured
  `Error::VersionMismatch { expected, found }` replacing string-based
  version mismatch (A-C2).
- dialect: op versioning + migration table (`Migration`,
  `Deprecation`, `AttrMap`, `Semver`) via `inventory::submit!`; chain
  resolution + deprecation diagnostics (A-C2b).
- perf: `BENCHMARKS.md` performance contract  -  10 targets, numerical
  stability per-op ULP bounds, regression gate spec (A-C14b).
- optimizer: `AdapterCaps` + `PassCtx` + `AnalysisCache`; typed-error
  conversion from `PassSchedulingError` to `Diagnostic` (A-C7b part 1).
- core: runtime introspection API  -  `dialects()`, `ops()`, `backends()`,
  `lowerings()`, `coverage_matrix()` (A-C11b).
- docs: op-id stability catalog + regen-on-demand gate
  (`docs/catalogs/op-id-catalog.md`); coverage matrix + regression gate
  (`docs/catalogs/coverage-matrix.md`) (A-B4d, A-C11b).
- scripts: layout / file-size / mod.rs-size / prelude / readmes CI
  law scripts under `scripts/laws/` (A-C11c part 1).

### Dialects (Gemini A)
- core: dialect foundation types  -  `OpDef`, `LoweringTable`,
  `DialectRegistry`, `InternedOpId`, `BackendRegistration` (A-B0).
- core: every Cat C intrinsic migrated to `naga::Module` builders  - 
  91 ops, zero shader assets remain in op trees (A-B1).
- core: primitive Cat A ops migrated; KAT coverage for 7 previously-
  missing programs (A-B2).
- core: `io` dialect  -  4 Cat C zero-copy intrinsics
  (`io.dma_from_nvme`, `io.write_back_to_nvme`, `mem.zerocopy_map`,
  `mem.unmap`) registered with no backend opt-in (B-B3 scope).

### Backends (Gemini B)
- wgpu: dispatch via `DialectRegistry.get_lowering`  -  `OpSpec::intrinsic`
  read path removed (B-B1).
- wgpu: `impl Executable` + `impl Compilable` for `WgpuBackend` with
  `WgpuIR` progressive-lowering artifact (B-B5).
- reference: `dialect_dispatch` module routes op ids through
  `DialectRegistry.get_lowering(CpuRef)` (B-B4).

### Performance (Gemini C)
- wgpu: lock-free `BufferPool` via crossbeam; `PrerecordedDispatch`
  pre-recording (C-B1).

### Pre-existing (landed earlier in the cycle)
- core: blake3 fingerprinting for IR stability and cache invalidation (MOD-008)
- core: arena-backed reference interpreter (P-2)
- runtime: zero-copy output-slice readback (P-5)
- runtime: streaming chunked dispatch (P-7)
- validator: tightened atomic indexes, fma/select typing, mixed arithmetic typing, and u64 bitwise-unary acceptance (VAL-001..004)
- conform: widened overflow-contract surface for primitive arithmetic regression coverage (CONF-001)
- conform: added build-scan regression coverage for generated operation metadata (CONF-002)
- wire: added depth-cap regression coverage for hostile nested IR blobs (EDGE-001)

### Changed
- `vyre-conform::specs::primitive` now walks `vyre::ops::registry` for every `primitive.*` op and builds specs from core metadata plus normalized `rules/kat/primitive/<family>/<op>.toml` vectors. Legacy per-op modules that were not present in the core registry, including `logical_and`, `logical_or`, `logical_xor`, `logical_nand`, `logical_nor`, `avg_floor`, `wrapping_neg`, and `popcount_sw`, were removed rather than kept as conform-only specs.

## [0.4.0-alpha.2]  -  2026-04-17

### Added
- Architecture and process contracts were formalized with `ARCHITECTURE.md`, `rules/SCHEMA.md#kat`, and `docs/PRIMITIVES.md`, giving a stable contributor contract for frozen traits, op classification, and community rulesets.
- New publishable package structure was established: `vyre-spec` (`0.1.0`) and `vyre-build-scan` (`0.1.0`) plus release-ready crate metadata for the workspace surface.
- Conformance foundations landed for this release with canonical `CpuOp` CPU reference plumbing in `core::ops::cpu_op`, `conform` pipeline cleanup, and the move of `reference` into `vyre` so evaluator semantics and wire-era tooling are co-located.
- Benchmark and evidence publishing pipeline landed: `primitives_showcase` entrypoint, `benches/RESULTS.md`, and synchronized benchmark presentation in README + book.

### Changed
- DeepPerf wave cleanup converted temporary tree-gen and generated-cruft artifacts into a stable one-file-per-op structure, including conform command/layout simplification and generated module deduplication.
- Core/conform import surfaces and type contracts were adjusted for category and registry stability, including `Category`/`IntrinsicTable` migration into `vyre-spec` and elimination of brittle cross exports.
- Documentation and validation semantics were tightened: `Fix:`-prefixed actionable diagnostics, contract-first doc language, and release-oriented invariant text for affected public surfaces.
- Package and build metadata was harmonized for publishability and release continuity.

### Fixed
- Fixed immediate compile/dependency coupling regressions from the prior refactor wave by removing dead or misleading generated surfaces and restoring stable compile boundaries.
- Fixed benchmark evidence drift by rebaselining published values from `benches/RESULTS.md` and aligning user-facing benchmark tables.
- Fixed stale release-state items by auditing all open coordination entries and refreshing statuses with explicit reopen criteria.

### Perf
- DeepPerf benchmark capture completed for primitive ops across 1K/10K/100K/1M element sizes with CPU and GPU end-to-end timings, crossover annotations, and the full 48-op table in `benches/RESULTS.md`.
- Preserved the end-to-end performance gate by excluding structural hacks and ensuring benchmark coverage remains tied to committed results data.
- Captured remaining hotspot context for future release polish (`gcd`, `lcm`, and uncovered KAT boundary classes) in coordination notes for targeted follow-up.

## [0.4.0-alpha.1]  -  previous

### Added
- Workspace merge of `vyre` core and `vyre-conform` into a single workspace.
- `SANTH_STANDARD.md` and `template_op.rs`  -  standardized contributor template for adding new ops (8fa6ab6, 436264b).
- `automod` wired across all op categories (bitwise, math, reductions, data_movement, string, scan, sort, encode, stats, buffer, compiler_primitives, rule, decode, match_ops, string_similarity, graph, workgroup, security_detection, hash) (c6af953, c4ab1f7, a39a9c5).
- CI workflow for check + clippy + doc (3c57a49).

### Changed
- Core consolidated from ~2000 files down to 1117 files with 0 compile errors (0956373, 5b6e1e5, 436264b).
- Conform merged and consolidated from 3645 files down to 883 files with 0 compile errors (09a6496).
- GPU feature gates stripped from conform; conform now assumes GPU is always available (ac760a8, b1b7991).

### Fixed
- Original 80-entry op registry restored after agent overwrites (b1b7991).
- Tree-gen damage consolidated and reverted where it broke the module graph (ade08d5, c91ad8c, 35f7342, dd71607).
