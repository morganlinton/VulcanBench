# vyre-debug: naga emit-failure triage spec

This spec covers gaps that block fast iteration on `vyre-emit-naga` /
`vyre-driver-wgpu` failures. Triage of a single naga validation /
WGSL writer failure today costs ~3–6 s rebuild + manual log
post-processing per attempt; this spec eliminates the
post-processing pass.

## What already exists

`vyre-debug` library + `vyre_dbg` CLI:

- `dump_descriptor` / `DumpDescriptor`  -  vyre-IR (descriptor) pretty-print
- `dump_wgsl` / `dump_wgsl_with_lines` / `DumpWgsl`  -  emits WGSL string
- `find_dangling_refs` / `FindDangling`  -  vyre-side dangling SSA refs
- `find_uncarriered_assigns` / `FindUncarriered`  -  Assigns inside loops not tagged
- `carrier_summary` / `CarrierSummary`  -  vyre-IR carrier analysis
- `bisect_rewrites` / `BisectRewrites`  -  find which rewrite pass breaks a descriptor
- `diff_descriptors` / `DiffDescriptors`  -  diff two descriptors
- `get_program`  -  hard-coded synthetic Programs (c11_lexer + a few others)

Everything above is **vyre-IR-side**. Naga-side and runtime-failure-side coverage is missing.

## What to add

### 1. Naga-module dump with resolved handles

`vyre_debug::dump_naga_module(module: &naga::Module) -> NagaDump`

Renders the full naga module with every handle pre-resolved against
the module's type arena. Replaces having to grep
`function_expressions = ?ep.function.expressions` from `tracing::trace!`
output. Output sections:

```
=== globals ===
[0] vyre_globals_in : array<u32, dynamic>, group=0 binding=0, ReadOnly
[1] vyre_globals_out : array<u32, dynamic>, group=0 binding=1, ReadWrite

=== local_variables ===
[301] vyre_loop_carry_873            : Bool
[376] vyre_loop_carry_198            : U32
[712] current_decl_parent_aggregate_scan_end : U32
[852] current_decl_parent_aggregate_scan_end : U32

=== expressions ===
[0]    FunctionArgument(0)                                    : vec3<u32>
[1]    AccessIndex { base: [0]/FunctionArgument(0), index: 0 } : u32
…
[6298] Load { pointer: [6297]/LocalVariable(852)/u32 }        : u32
[6299] Load { pointer: [6299_p]/LocalVariable(301)/Bool }     : Bool
…

=== body ===
{
  Emit([1..2])
  Store {
    pointer: [12]/LocalVariable(0)/U32 "tok_len"
    value:   [11]/Literal(U32(0))      :U32
  }
  Loop {
    body: {
      …
    }
    continuing: {
      Emit([6300..6303])
      Store {
        pointer: [6303]/LocalVariable(852)/U32 "current_decl_parent_aggregate_scan_end"
        value:   [6302]/Load(LocalVariable(301)/Bool) :Bool   <-- TYPE MISMATCH
      }
    }
    break_if: …
  }
  …
}
```

`NagaDump` exposes `to_string()` and a `find(handle: u32)` method that
returns the resolved expression with its enclosing block path
(`["root", "Loop@line=N", "continuing"]`).

### 2. Failure-localized handle trace

`vyre_debug::failure_trace(module: &naga::Module, error: &naga::valid::ValidationError) -> FailureTrace`

For every `Handle<Expression>` named in the validation error,
recursively chase operand handles AND find every `Statement` (in any
block) that references that handle, reporting:

```
FAILURE: InvalidStoreTypes { pointer: [6303], value: [6302] }

  pointer [6303] = LocalVariable(852)
    local 852 "current_decl_parent_aggregate_scan_end" : U32
    bound from: <reverse-lookup via bind_result_log if attached, else "unknown">

  value [6302] = Load { pointer: [6301] }
    [6301] = LocalVariable(301)
    local 301 "vyre_loop_carry_873" : Bool
    bound from: vyre op id 873 (LoopIndex { loop_var: "current_decl_parent_aggregate_scan_end" }), bind_result_typed ty=U32, but allocate_carrier_local picked Bool because value_types[873] was Bool at allocation time (set by op id 871 / BinOp(Eq) before id 873 reused this name)

  use-site: Statement::Store at body[2].Loop.continuing[3]
    enclosing Emit ranges: 6300..6303
    enclosing block path: ["root", "Loop@body[2]", "continuing"]
```

### 3. Wgsl-writer failure trace

`vyre_debug::failure_trace_wgsl(module: &naga::Module, info: &ModuleInfo, err: &naga::back::wgsl::Error) -> FailureTrace`

Same shape as #2 but for the WGSL writer's `no definition in scope
for identifier _eN` family. Walk to the let-binding's emit position
and report the path between let-binding scope and use scope so it's
obvious where a `LocalVariable + Load` round-trip is missing.

### 4. `bind_result` capture log

`vyre_emit_naga` exposes (behind a feature gate or env var
`VYRE_BIND_RESULT_LOG`) a side-channel that records every
`bind_result` call:

```
struct BindResultEntry {
    vyre_op_id: u32,
    op_kind: String,          // Debug-formatted KernelOpKind
    init_handle: Handle<Expression>,
    init_scalar_kind: Option<ScalarKind>,
    child_body_depth: usize,
    value_types_at_call: Option<Handle<Type>>,
    publish_path: PublishPath, // Inline | LoopCarrier { local } | BlockScoped { local }
    local_allocated_ty: Option<Handle<Type>>,
}
```

`vyre_debug::load_bind_result_log(path) -> Vec<BindResultEntry>` lets
the failure trace cross-reference naga handles with vyre op ids  - 
the manual `eprintln!("[publish]…")` step I'm doing now.

### 5. Capture-on-failure for vyrec

`vyrec` (or `vyre-frontend-c::api::compile`) honors
`VYRE_CAPTURE_FAILED_DESCRIPTOR=/path/dir`. On dispatch failure of
any kernel, serialize the in-flight `KernelDescriptor` (or `Program`
+ `descriptor_for`) to `<dir>/<kernel-name>.kdesc.bin` and the
naga::Module to `<dir>/<kernel-name>.module.ron` (if it got that far).

`vyre_debug emit-replay --kdesc <path>` then reproduces the failure
without running the whole frontend pipeline. Today I cannot capture
the failing `c11_annotate_typedef_names` descriptor without a
deep-frontend run that takes ~10 s for trivial inputs.

### 6. `vyre_dbg failure-trace`

Top-level CLI:

```
vyre_dbg failure-trace --kdesc /tmp/c11_annotate.kdesc.bin
```

Loads the descriptor, runs `emit_optimized`, validates, and on
failure prints the full failure-trace report. With
`--bind-result-log /tmp/bind_log.bin` the report includes vyre-op-id
correlation. Default output is human-readable; `--json` for
machine-readable.

### 7. `vyre_dbg emit-replay`

Same loading path as `failure-trace` but always dumps:

```
out/
├── descriptor.txt        # vyre IR (existing dump_descriptor)
├── module.txt            # naga module (NagaDump from #1)
├── module.ron            # naga::Module via ron, machine-readable
├── shader.wgsl           # WGSL output (success or partial)
└── failure.txt           # FailureTrace if validation/writer failed
```

### 8. `vyre_dbg diff-emit`

```
vyre_dbg diff-emit --kdesc-a /a.kdesc --kdesc-b /b.kdesc
```

Renders both via `module.txt` and runs `git diff --no-index`. Lets me
see the exact handle-level + statement-level delta when toggling a
fix in `vyre-emit-naga` on/off.

### 9. `vyre_dbg find-uses-of-handle`

```
vyre_dbg find-uses-of-handle --kdesc /failing.kdesc --handle 9873
```

Loads, emits, and reports every Statement and Expression that
references handle [9873], with enclosing block paths. Today I'm
running `python3 -c 'for m in re.finditer(...)'` against trace logs.

### 10. `vyre_dbg find-uses-of-vyre-op`

Same as #9 but indexed by vyre op id (requires bind-result log). Lets
me ask "which naga statements consume the result of vyre op 873?"
without manually walking the bind_result map.

### 11. `vyre_dbg pipeline-cache-clear`

```
vyre_dbg pipeline-cache-clear
```

Wraps `rm -rf ~/.cache/vyre/pipeline`. The disk-backed pipeline cache
serves stale WGSL across vyrec rebuilds and made me chase phantom
"my fix didn't take effect" failures three times this session.

## Implementation notes for the upgrade agent

- Type resolution: `module.types[handle]` exposes `TypeInner`. For
  `Scalar(Scalar { kind, width })` render as `Bool/U32/I32/F32/U64/F64`.
  For `Vector { size, scalar }`, `Atomic(scalar)`, `Array { base, size, stride }`,
  `Struct { members, span }`, render structurally  -  those show up.
- The four canonical handles (`bool_ty/u32_ty/i32_ty/f32_ty`) are NOT
  the only typed locals. `loops.rs:131` allocates a local with
  `name: Some(loop_var.to_string()), ty: index_ty` where `index_ty`
  may not equal any cached handle (it's whatever
  `value_type_operand` returned). This is the source of the
  "binding_types_lookup returns None" failure mode: resolve via
  `module.types[ty]` directly, not just the cache.
- Block-path tracking: walk `function.body` recursively, emitting a
  `["root", "If@accept[N]", "Loop.body", ...]` path string for every
  Statement. Use this in #2 and #9.
- Handle-use index: build a `HashMap<Handle<Expression>, Vec<UseSite>>`
  by walking every Statement in every block once. UseSite holds the
  block path + the Statement variant + the operand role
  (`StoreValue`, `StorePointer`, `IfCondition`, `LoopBreakIf`,
  `BinaryLeft`, `BinaryRight`, `SelectAccept`, `SelectReject`,
  `SelectCondition`, `AccessBase`, `AccessIndex`, `LoadPointer`,
  `EmitRange{first,last}`).
- Reverse vyre-op lookup: requires `vyre-emit-naga` to write the
  bind-result log. Suggest `OnceCell<Mutex<Vec<BindResultEntry>>>`
  guarded by `cfg(feature = "bind-result-log")` or env var
  `VYRE_BIND_RESULT_LOG=/path/file` activated at module init.
- Capture-on-failure: vyrec invocation path runs through
  `vyre_frontend_c::api::compile`. Easiest hook is
  `vyre-driver-wgpu`'s `compile_compute_pipeline_with_layout`
  (already has `dump_wgsl_if_requested`); add a
  `dump_kdesc_if_requested` sibling driven by
  `VYRE_DUMP_KDESC` that writes the upstream `KernelDescriptor`. The
  Program → KernelDescriptor lowering is in `vyre-lower::lower`.

## Out of scope

- Source-level vyre-IR rewrites or fixes  -  the agent works on
  vyre-debug only.
- Naga upstream patches.
- Adding new kernels to the `get_program` helper unless trivially
  needed by the spec above.
