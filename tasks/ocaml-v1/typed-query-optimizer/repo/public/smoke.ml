open Expr
let () =
  let reads=ref [] in
  let read key=reads:=key::!reads; if key="price" then 12 else 7 in
  let plan=Let(Read "price",Add(Var Z,Var Z)) in
  assert(Eval.run ~read () plan=24);
  assert(!reads=["price"]);
  assert(Eval.run ~read () (Optimizer.normalize plan)=24);
  assert(Eval.run ~read (3,()) (Let(Int 4,Add(Var Z,Var(S Z))))=7)
