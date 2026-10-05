let decode : type a.
  a Schema.t -> string -> pos_ref:int ref -> limit:int -> (a, Error.t) result
  =
  fun schema data ~pos_ref ~limit ->
    Schema.validate schema;
    (let start = !pos_ref in
     if (start < 0) || ((limit < start) || (limit > (String.length data)))
     then Error { Error.at = start; path = []; reason = Invalid_bounds }
     else
       (match schema with
        | Schema.Bool when
            (limit = (start + 1)) &&
              (((data.[start]) = '\000') || ((data.[start]) = '\001'))
            -> (pos_ref := limit; Ok ((data.[start]) = '\001'))
        | _ -> Error { Error.at = start; path = []; reason = Truncated }))
