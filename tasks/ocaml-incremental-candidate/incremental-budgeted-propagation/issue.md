# Budgeted propagation in the full Incremental repository

Implement the supplied stabilize_with_budget API in this complete pinned
Incremental v0.17.0 checkout. This is an original extension, not a report of an
upstream defect. The prototype always returns More. All existing clients and
behavior outside the new contract must continue working. Build and test with
dune build --profile release src/incremental.cmxa test/incremental_test.cmxa
@runtest. Dependencies are installed offline. The public clients execute and
the upstream test library compiles; upstream debug/inline test execution is not
part of this portable target.

A positive max_recomputations quota caps the actual increase in
State.num_nodes_recomputed during this call, not heap pops, changed nodes,
callbacks, or wall time. Every recomputed node counts, including internal
bind/join/if nodes and direct-parent optimizations. A nonpositive quota raises
Invalid_argument before any state change, even on a paused or poisoned state.
Starting a cycle and finishing its publication are not recomputations.
Consume up to the quota unless the recompute heap becomes empty. Return More
exactly when stale necessary work remains; return Done in the call that
exhausts it, including an empty cycle. Do not require an extra empty slice.

More leaves the same stabilization cycle in progress. Repeated slices resume
it without relinking observers or incrementing num_stabilizes. A caller may
change quotas between calls. Existing Expert.do_one_step_of_stabilize may
resume a paused budgeted cycle and the budgeted API may resume a stepped
cycle. Existing stabilize must also finish a paused cycle without starting
another one. Ordinary one-step startup/Keep_going behavior remains unchanged.
Keep state local to each graph, including generic State.create and Make APIs;
a callback in one graph may drive a different graph.

Preserve dependency order and cutoffs for diamonds, dynamic bind/join/if
branches, invalidation, and expert nodes. Never restart the computation or
discard pending nodes to obey the quota. A More boundary is a consistent
scheduler boundary, not an exception through a node callback. Preserve
normal observer restrictions: value_exn is forbidden while propagation is
paused, and on_update handlers run only once at cycle completion. New
observers made during propagation become active in the next cycle.
Var.set during propagation is deferred and last-write-wins, even between
slices; watch values in the current cycle use the entry value. The queued
writes take effect after publication and require the next cycle to propagate.

All three driver entry points (stabilize, stabilize_with_budget, and expert
step) must reject a same-graph call from an executing node or update handler
before changing scheduler state. A caught rejection does not poison the
graph or consume another recomputation. Calls between slices are valid.
An uncaught callback exception still propagates the same exception object
and permanently poisons the graph as in the original API. Subsequent driver
calls must reject without recomputation or publication. Invalid budget and
reentrancy validation must not replace the original stored failure.

Preserve every supplied .mli and src/incremental_intf.ml byte for byte.
Preserve supplied test/ and public/ files as well.
Keep ordinary unbounded direct-parent optimizations enabled, as witnessed
by both existing direct-recomputation counters on suitable graphs. A sliced
implementation may queue those parents to meet its quota. Add no unsafe
casts, hidden global state registry, or dependency/network downloads. Existing
upstream unsafe operations may stay. No requirement limits callback duration
or makes this API safe for concurrent threads.
