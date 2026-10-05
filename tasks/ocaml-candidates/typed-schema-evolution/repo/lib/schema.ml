type 'a presence =
  | Required 
  | Default of 'a 
type _ t =
  | Bool: bool t 
  | Int64: int64 t 
  | Text: string t 
  | Pair: 'a t * 'b t -> ('a * 'b) t 
  | Option: 'a t -> 'a option t 
  | List: 'a t -> 'a list t 
  | Record: ('a, 'r) fields * ('a -> 'r) -> 'r t 
and (_, _) fields =
  | Nil: (unit, 'r) fields 
  | Field: int * 'a t * 'a presence * ('r -> 'a) * ('b, 'r) fields ->
  (('a * 'b), 'r) fields 
let rec validate : type a. a t -> unit =
  function
  | Bool | Int64 | Text -> ()
  | Pair (a, b) -> (validate a; validate b)
  | Option a -> validate a
  | List a -> validate a
  | Record (fs, _) -> let seen = Hashtbl.create 8 in fields seen fs
and fields : type a r. (int, unit) Hashtbl.t -> (a, r) fields -> unit =
  fun seen ->
    function
    | Nil -> ()
    | Field (id, s, _, _, rest) ->
        (if (id < 1) || ((id > 65535) || (Hashtbl.mem seen id))
         then invalid_arg "schema field id";
         Hashtbl.add seen id ();
         validate s;
         fields seen rest)
