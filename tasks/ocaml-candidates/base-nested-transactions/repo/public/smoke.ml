let () =
  let t = Base.Hashtbl.create (module Base.Int) in
  Base.Hashtbl.set t ~key:1 ~data:2;
  assert
    ((Base.Hashtbl.with_transaction t
        ~f:(fun () -> Base.Hashtbl.find_exn t 1))
       = 2)
