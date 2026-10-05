let () =
  let q = { Domain.id = 7L; label = "old"; urgent = false } in
  let old = Codec.encode Domain.v1 q in
  match Codec.decode Domain.v2 old ~pos_ref:(ref 0)
          ~limit:(String.length old)
  with
  | Ok q -> assert (not q.Domain.urgent)
  | Error _ -> failwith "migration missing"
