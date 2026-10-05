open Bin_prot
let () =
 let b=Utils.bin_dump ~header:true Type_class.bin_writer_int 42 in
 assert (Bigarray.Array1.dim b = 9);
 let p=ref 8 in assert (Read.bin_read_int b ~pos_ref:p = 42);
 let s=Tagged.Pair (Tagged.Atom Type_class.bin_int, Tagged.Option (Tagged.Atom Type_class.bin_bool)) in
 let wire=Tagged.encode s (42,Some true) in
 assert (Bigarray.Array1.dim wire=15);
 assert ((Tagged.writer s).size (42,Some true) = 15)
