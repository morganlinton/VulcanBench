let () =
  let t = Base.Hashtbl.Poly.create () in
  let result =
    Base.Hashtbl.Poly.with_transaction t
      ~f:(fun () ->
            Base.Hashtbl.Poly.with_transaction t
              ~f:(fun () -> Base.Hashtbl.Poly.set t ~key:"item" ~data:3);
            42) in
  assert ((result = 42) && ((Base.Hashtbl.Poly.length t) = 1))
