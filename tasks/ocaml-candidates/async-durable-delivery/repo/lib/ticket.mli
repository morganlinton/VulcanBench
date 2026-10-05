type t
val create : int -> t
val seq : t -> int
val result : t -> Outcome.completion Async_kernel.Deferred.t
val cancel : t -> unit
val on_cancel : t -> (unit -> unit) -> unit
val settle : t -> Outcome.completion -> unit
