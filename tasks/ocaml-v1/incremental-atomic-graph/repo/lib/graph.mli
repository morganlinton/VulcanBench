type t
type subscription
val create : (string * int) list -> t
val transaction : t -> sources:(string * int) list -> formulas:(string * Expression.t) list -> (unit,string) result
val subscribe : t -> string -> subscription option
val stabilize : t -> unit
val value : subscription -> int
val additions : t -> int
