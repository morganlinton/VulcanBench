let () =
  let t = Base.Hashtbl.create (module Base.String) in
  Base.Hashtbl.with_transaction t
    ~f:(fun () ->
          Base.Hashtbl.set t ~key:"cache" ~data:4;
          Base.Hashtbl.incr t "cache");
  assert ((Base.Hashtbl.find_exn t "cache") = 5)
