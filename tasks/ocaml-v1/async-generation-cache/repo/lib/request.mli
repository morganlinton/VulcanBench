type 'a outcome = Value of 'a | Failed of string | Timed_out | Cancelled | Closed
type 'a t
val result : 'a t -> 'a outcome Async_kernel.Deferred.t
val cancel : 'a t -> unit
val is_pending : 'a t -> bool
(* Engine plumbing. Resolve is first-result-wins and does not call on_cancel.
    Cancel resolves Cancelled before invoking the registered hook, once. *)
val create : unit -> 'a t
val resolve : 'a t -> 'a outcome -> unit
val on_cancel : 'a t -> (unit -> unit) -> unit
