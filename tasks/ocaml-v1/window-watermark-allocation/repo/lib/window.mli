type t
val create : width:int -> t
val add : t -> time:int -> value:int -> (unit,string) result
val total : t -> int
val count : t -> int
val watermark : t -> int option
val bucket_visits : t -> int
