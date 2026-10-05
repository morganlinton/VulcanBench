module S = (val Incremental.State.create ())
let () =
  let v = Incremental.Var.create S.t 4 in
  let o = Incremental.observe (Incremental.map (Incremental.Var.watch v) ~f:succ) in
  Incremental.stabilize S.t;
  assert (Incremental.Observer.value_exn o = 5)
let _bounded_api = Incremental.stabilize_with_budget ~max_recomputations:1 S.t
