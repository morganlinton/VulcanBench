type t
type 'a handle
type visitor = { visit : 'a. 'a Key.t -> 'a -> unit }
val create : unit -> t
val register : t -> 'a Key.t -> 'a -> 'a handle
val find : t -> 'a Key.t -> 'a option
val handle : t -> 'a Key.t -> 'a handle option
val read : 'a handle -> 'a option
val update : 'a handle -> ('a -> 'a) -> bool
val remove : t -> 'a Key.t -> bool
val close : t -> unit
val clone : t -> t
val transfer : src:t -> dst:t -> 'a Key.t -> bool
val iter : t -> visitor -> unit
