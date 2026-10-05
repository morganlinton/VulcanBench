type t
type handle
val create : Async_kernel.Time_source.t -> t
val at : t -> Core.Time_ns.t -> (unit -> unit) -> handle
val cancel : handle -> unit
val pending : t -> int
