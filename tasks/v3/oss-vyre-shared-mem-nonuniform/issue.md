# Shared-memory promotion inserts workgroup barriers inside non-uniform control flow

A lowering rewrite promotes repeated global U32 loads into a per-workgroup
cooperative copy into shared memory. When it promotes, it inserts `AsyncLoad` /
`AsyncWait` and a workgroup `Barrier`.

That rewrite currently walks into **every** child body, including bodies that
only some lanes enter (`StructuredIfThen` / `StructuredIfThenElse` /
`StructuredForLoop`). A workgroup barrier (or cooperative async copy) in
non-uniform control flow is illegal on WGSL / CUDA / SPIR-V: lanes that did not
take the branch never reach the barrier, so the workgroup deadlocks or hits UB.
The common shape `if (gid < n) { x = load(g, gid); y = load(g, gid); }` is
enough to trigger it.

## Expected behavior

- Promote (and therefore insert cooperative copy + workgroup barrier) **only**
  in bodies that every lane in the workgroup reaches.
- The kernel entry body is workgroup-uniform. Unconditional grouping ops such as
  `StructuredBlock` / `Region` preserve uniformity into their children.
- Bodies entered under `StructuredIfThen`, `StructuredIfThenElse`, or
  `StructuredForLoop` are non-uniform: recursion may continue for nested
  analysis, but must not insert Barrier / AsyncLoad / AsyncWait there.
- Root-body promotion of repeated uniform global/local-id loads must keep
  working.

For a descriptor whose only repeated loads live inside a `StructuredIfThen`
child, the rewritten child must still contain exactly those loads and must not
gain Barrier / AsyncLoad / AsyncWait ops.
