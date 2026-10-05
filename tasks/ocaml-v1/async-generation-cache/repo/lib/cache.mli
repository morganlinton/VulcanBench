type 'a t
val create : source:Async_kernel.Time_source.t -> policy:Policy.t ->
  worker:(string -> attempt:int -> ('a,string) result Async_kernel.Deferred.t) -> 'a t
val get : 'a t -> string -> 'a Request.t
val invalidate : 'a t -> string -> unit
val close : 'a t -> unit
val in_flight : 'a t -> int
val cached : 'a t -> int
val pending_alarms : 'a t -> int
