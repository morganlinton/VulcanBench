let rec payload : type a. a Schema.t -> a -> string =
  fun schema value ->
    let b = Buffer.create 32 in
    (match schema with
     | Schema.Bool -> Buffer.add_char b (if value then '\001' else '\000')
     | Int64 -> Buffer.add_string b (Wire.int64 value)
     | Text -> Buffer.add_string b value
     | Pair (a, c) ->
         (Wire.framed b (payload a (fst value));
          Wire.framed b (payload c (snd value)))
     | Option s ->
         (match value with
          | None -> Buffer.add_char b '\000'
          | Some v -> (Buffer.add_char b '\001'; Wire.framed b (payload s v)))
     | List s ->
         (Wire.u16 b (List.length value);
          List.iter (fun v -> Wire.framed b (payload s v)) value)
     | Record (fs, _) ->
         let entries =
           List.sort (fun (a, _) (c, _) -> compare a c) (entries fs value) in
         (Wire.u16 b (List.length entries);
          List.iter (fun (id, s) -> Wire.u16 b id; Wire.framed b s) entries));
    Buffer.contents b
and entries : type a r. (a, r) Schema.fields -> r -> (int * string) list =
  fun fs value ->
    match fs with
    | Nil -> []
    | Field (id, s, _, get, rest) -> (id, (payload s (get value))) ::
        (entries rest value)
let encode schema value = Schema.validate schema; payload schema value
