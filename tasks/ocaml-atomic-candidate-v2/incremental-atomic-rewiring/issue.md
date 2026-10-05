# Atomic expert dependency rewiring

Implement Expert.Node.apply_dependency_edits in the full Incremental library.
The supplied generative and generic signatures are fixed. Each Replace (node,
dependencies) supplies the complete ordered dependency list for an expert node.
One batch may edit nodes with different value types. Dependencies within one
replacement list have a common child value type, independent of the parent's
value type. The generic API takes the state explicitly; the generative API uses
its module's state. The source includes an unsolved stub.

Validate the complete prospective graph before any mutation. A batch can reverse
an edge between already observed expert nodes, with the obsolete edge removed
in the same batch. Application order must not introduce a temporary cycle.
Cycles must be detected through ordinary Incremental nodes too, and through
unobserved expert nodes. A self-edge is a cycle. The existing graph, with all
replacement lists substituted simultaneously, defines the prospective graph.
Do not validate by adding edges to the live graph and attempting rollback.

Return Ok () on success. Return the following exact Error strings in precedence
order, without changing any topology, ownership, staleness, values, necessity,
callbacks or future propagation on rejection:

1. "busy": the state is stabilizing (including a paused expert-step cycle),
   running update handlers, or poisoned by a previous stabilization exception.
   This check applies even to an empty batch.
2. "duplicate node": a target node occurs more than once.
3. "duplicate dependency": a dependency object occurs more than once across
   all supplied lists. Distinct dependency objects for the same child are legal.
4. "invalid node": a target is invalid or is not an expert node; any node reached
   from the targets in the prospective child graph is invalid, in another state,
   or was created inside a bind scope. This API deliberately supports top-scope
   graphs only. Validate unreachable-from-observers targets as well.
5. "dependency in use": a supplied dependency is already attached to a parent
   outside the batch. An attached dependency can remain with its current parent
   or transfer to another target if its old parent omits it in the same batch.
6. "cycle": the prospective graph contains a cycle reachable from a target.

A dependency omitted by its old parent is detached after success and can be
reused later. Retained and transferred objects keep their identity. Their
internal indices, parent backlinks and necessity bookkeeping must stay coherent.
An unchanged ordered list of the same objects is a no-op: it must not cause
recomputation, callbacks or observability transitions. A reorder is a change.
An empty idle batch is also a no-op.

Successful application does not evaluate node functions or publish observer
updates. Previously stabilized observer values remain readable until the next
stabilization. For each changed expert, all its new dependency on_change callbacks
must run in the supplied order before its next node function, using current
child values. None of that expert's dependency callbacks may run during the batch.
Removed dependencies must not receive callbacks from subsequent child changes.
Normal callback delivery on later child changes remains intact. The usual
idempotence requirement applies. Batch application may cause the normal
on_observability_change transitions while disconnecting and reconnecting nodes;
those callbacks must neither mutate the graph nor raise. Reentrant batch calls
from those callbacks must return "busy".

Necessary changed nodes must recompute at the next stabilization; unobserved
nodes remain lazy. Cutoffs still suppress unchanged downstream work. Preserve
ordinary map, variable, expert dependency and step-driver behavior. Rejection
must not poison the state; a later valid batch and stabilization must work.
This is a topology transaction, not a transaction over user-maintained variables
in node closures. Clients must update their closure's list after a successful
replacement, before stabilizing.

As with the existing graph APIs, callers must provide enough max-height headroom
for height adjustment. Allocation failures, exhaustion of that resource, and
violations of the callback preconditions above are outside the Error/rollback
contract. Do not replace the graph with fresh nodes, use unsafe casts, disable
cutoffs, forge counters or bypass Incremental's propagation machinery.

Preserve all supplied .mli files, src/incremental_intf.ml, upstream test fixtures,
and public clients. Implement the solution in other .ml files. The portable
preflight builds src/incremental.cmxa and test/incremental_test.cmxa and executes
public clients via @runtest. It does not run every upstream debug/inline test.
