module I = Incremental.Make ()
let () =
  let v = I.Var.create 2 in
  let o = I.observe (I.map (I.Var.watch v) ~f:succ) in
  I.stabilize (); assert (I.Observer.value_exn o = 3);
  I.Var.set v 8; I.stabilize (); assert (I.Observer.value_exn o = 9)
let _atomic_api = I.Expert.Node.apply_dependency_edits
