val dump : 'a Tagged.t -> 'a -> Common.buf
val decode_next : 'a Tagged.t -> Common.buf -> pos_ref:int ref -> limit:int -> max_size:int -> ('a option, Tagged.error) result
