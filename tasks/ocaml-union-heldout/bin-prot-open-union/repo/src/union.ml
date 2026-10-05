type 'r case = Case : { tag : int; codec : 'a Tagged.t; project : 'r -> 'a option; inject : 'a -> 'r } -> 'r case
type 'r unknown = { project_unknown : 'r -> (int * Common.buf) option; inject_unknown : int -> Common.buf -> 'r }
type 'r t = Closed of 'r case list | Open of 'r case list * 'r unknown
type reason = Invalid_bounds | Truncated | Invalid_tag | Unknown_tag of int | Trailing | Payload of Tagged.error
type error = { at : int; reason : reason }
exception Decode of error
let cases = function Closed xs | Open (xs, _) -> xs
let validate schema =
 let seen = Hashtbl.create 8 in
 List.iter (fun (Case c) ->
   if c.tag < 1 || c.tag > 65535 || Hashtbl.mem seen c.tag then invalid_arg "Union tag";
   Hashtbl.add seen c.tag ()) (cases schema);
 List.iter (fun (Case c) -> Tagged.validate c.codec) (cases schema)
let encode schema _ = validate schema; failwith "Union.encode pending"
let decode schema _ ~pos_ref:_ ~limit:_ = validate schema; failwith "Union.decode pending"
let writer schema : _ Type_class.writer =
 { size=(fun v -> Bigarray.Array1.dim (encode schema v));
   write=(fun _ ~pos:_ _ -> failwith "Union.write pending") }
let reader schema : _ Type_class.reader =
 { read=(fun b ~pos_ref -> match decode schema b ~pos_ref ~limit:(Bigarray.Array1.dim b) with Ok v -> v | Error e -> raise (Decode e));
   vtag_read=(fun _ ~pos_ref:_ _ -> invalid_arg "Union vtag") }
let type_class ~shape schema : _ Type_class.t = {shape; writer=writer schema; reader=reader schema}
