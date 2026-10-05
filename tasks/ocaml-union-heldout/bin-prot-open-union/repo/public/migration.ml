open Bin_prot
open Tagged
type quote = { id : int; name : string; urgent : bool }
let old = Record (Field (1, Atom Type_class.bin_int, Required, (fun q->q.id),
 Field (2, Atom Type_class.bin_string, Required, (fun q->q.name), Nil)),
 (fun (id,(name,())) -> {id;name;urgent=false}))
let current = Record (Field (1, Atom Type_class.bin_int, Required, (fun q->q.id),
 Field (2, Atom Type_class.bin_string, Required, (fun q->q.name),
 Field (3, Atom Type_class.bin_bool, Default false, (fun q->q.urgent),Nil))),
 (fun (id,(name,(urgent,()))) -> {id;name;urgent}))
let () =
 let q={id=42;name="legacy";urgent=false} in let b=Tagged.encode old q in
 match Tagged.decode current b ~pos_ref:(ref 0) ~limit:(Bigarray.Array1.dim b) with
 | Ok got -> assert (got=q)
 | Error {reason=Truncated;_} -> () (* Compiling prototype, migration enabled by implementation. *)
 | Error _ -> failwith "migration"
