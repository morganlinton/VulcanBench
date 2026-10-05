type t
val create : unit -> t
val feed : t -> string -> (Wire.packet,string) result list
val finish : t -> (unit,string) result
val reset : t -> unit
