type t
val create : concurrency:int -> capacity:int -> worker:(int -> (int,string) result Async_kernel.Deferred.t) -> t
val submit : t -> int -> (int,string) result Async_kernel.Deferred.t
val close : t -> unit Async_kernel.Deferred.t
val abort : t -> unit Async_kernel.Deferred.t
val active : t -> int
val queued : t -> int
