val decode :
  'a Schema.t ->
    string -> pos_ref:int ref -> limit:int -> ('a, Error.t) result
