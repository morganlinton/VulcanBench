# Contract coverage

| Group | Stated contract |
| --- | --- |
| exact_quota | Actual node counts, recursive chains, multi-child parents, varying quotas, same-call completion and empty cycles |
| dynamic_dependencies | Bind scopes and invalidation, join, if, diamonds, cutoffs, expert dependencies and invariants |
| publication_fence | No observer values or update delivery while paused, exactly one publication, observers created during propagation activate next cycle |
| deferred_writes | Between-slice and callback writes are deferred, last writer wins, current and next-cycle values stay distinct |
| driver_interop | Legacy full and one-step drivers resume the same cycle, generic witnesses and distinct graph states |
| reentrancy | All driver combinations reject same-graph callback and handler calls; caught errors leave the outer graph healthy; another graph is valid |
| poison_and_validation | Invalid quota changes nothing, original callback exception identity, permanent poisoning, no future work or failure replacement |
| ordinary_workflow | Existing client values and both unbounded direct-parent counters after suitable updates |
| supplied_interfaces | Every supplied .mli, src/incremental_intf.ml, upstream test/ and public/ fixtures retain their exact hashes |

Four faulty controls must compile, retain regression guards and fail behavior:
heap-pop-quota, extra-empty-slice, step-accepts-reentrancy, restart-paused-cycle.
No model patch or reference may be admitted on a compiler failure alone.
The gate repeats three fresh offline base/reference pairs before controls and
the standard validator. Public build setup occurs before hidden injection.
Tests assert semantic behavior without prescribing a particular implementation.
