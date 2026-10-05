type t
type alarm
val create : unit -> t
val now : t -> int
val at : t -> int -> (unit -> unit) -> alarm
val cancel : alarm -> unit
val advance : t -> int -> unit
val pending : t -> int
