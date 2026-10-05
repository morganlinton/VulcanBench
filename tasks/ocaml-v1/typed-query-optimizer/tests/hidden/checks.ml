open Expr

let fingerprint = Measure.fingerprint
let expect term text =
  let actual=fingerprint term in
  if actual<>text then failwith (Printf.sprintf "expected %s, got %s" text actual)

let outcome env term stop =
  let trace=ref [] in
  let counter=ref 0 in
  let read key =
    trace:=key::!trace;
    let i= !counter in incr counter;
    if i=stop then failwith "injected reader failure";
    String.length key * 7 + i
  in
  let result = try Ok(Eval.run ~read env term) with Failure text -> Error text in
  result,List.rev !trace

let equivalent env term =
  let optimized=Optimizer.normalize term in
  List.iter (fun stop -> assert(outcome env term stop=outcome env optimized stop)) [-1;0;1;2;7];
  assert(fingerprint optimized=fingerprint(Optimizer.normalize optimized));
  optimized

let rec generated : type env. Random.State.t -> int -> (env,int) t = fun rng depth ->
  if depth=0 then
    if Random.State.bool rng then Int(Random.State.int rng 21-10)
    else Read(string_of_int(Random.State.int rng 4))
  else
    let g ()=generated rng (depth-1) in
    match Random.State.int rng 10 with
    | 0 -> Add(g(),g()) | 1 -> Mul(g(),g())
    | 2 -> If(Equal(g(),g()),g(),g())
    | 3 -> Fst(Pair(g(),g())) | 4 -> Snd(Pair(g(),g()))
    | 5 -> Let(g(),Add(Var Z,g()))
    | 6 -> Let(g(),Add(Var Z,Var Z))
    | 7 -> Let(g(),g())
    | 8 -> Let(Pair(g(),g()),Add(Fst(Var Z),Snd(Var Z)))
    | _ -> If(Bool(Random.State.bool rng),g(),g())

let () = match Sys.argv.(1) with
| "interpreter_contract" ->
  let term=Let(Read "x",Fst(Pair(Add(Var Z,Var Z),Read "discarded"))) in
  assert(outcome () term (-1)=(Ok 14,["x";"discarded"]));
  assert(outcome () term 1=(Error "injected reader failure",["x";"discarded"]));
  assert(Eval.run ~read:(fun _ -> assert false) (3,())
    (Let(Bool true,If(Var Z,Var(S Z),Int 8)))=3)
| "renaming_scope" ->
  let shift : (int * unit,bool * (int * unit)) Optimizer.renaming =
    {apply=(fun v -> S v)} in
  let term=Let(Int 5,Let(Bool true,If(Var Z,Add(Var(S Z),Var(S(S Z))),Int 0))) in
  let renamed=Optimizer.rename shift term in
  expect renamed "let(i5,let(btrue,if(v0,+(v1,v3),i0)))";
  assert(Eval.run ~read:(fun _ -> assert false) (false,(9,())) renamed=14);
  let swapped : (int * (bool * unit),bool * (int * unit)) Optimizer.renaming =
    {apply=(fun (type a) (v:(int * (bool * unit),a) var) : (bool * (int * unit),a) var ->
      match v with Z->S Z | S Z->Z | S(S _) -> .)} in
  let term=Let(Read "x",If(Var(S(S Z)),Add(Var Z,Var(S Z)),Int 0)) in
  let term=Optimizer.rename swapped term in
  assert(Eval.run ~read:(fun _ -> 11) (true,(7,())) term=18)
| "simultaneous_substitution" ->
  let s : (int * (int * unit),int * (int * unit)) Optimizer.substitution =
    {replace=(fun (type a) (v:(int * (int * unit),a) var) : (int * (int * unit),a) t ->
      match v with Z->Add(Var(S Z),Int 1) | S Z->Var Z
      | S(S _) -> .)} in
  let term=Let(Int 4,Let(Read "r",Add(Var(S Z),Add(Var(S(S Z)),Var(S(S(S Z))))))) in
  let got=Optimizer.substitute s term in
  expect got "let(i4,let(r\"r\",+(v1,+(+(v3,i1),v2))))";
  assert(Eval.run ~read:(fun _ -> 19) (3,(8,())) got=16);
  let closed : (int * unit,unit) Optimizer.substitution =
    {replace=(fun (type a) (v:(int * unit,a) var) : (unit,a) t ->
      match v with Z->Read "free" | S _ -> .)} in
  let got=Optimizer.substitute closed (Let(Read "bound",Add(Var Z,Var(S Z)))) in
  assert(outcome () got (-1)=(Ok 64,["bound";"free"]))
| "literal_beta" ->
  let term=Let(Pair(Int 4,Bool true),
    Let(Read "fee",If(Snd(Var(S Z)),Add(Fst(Var(S Z)),Add(Var Z,Int 0)),Int 99))) in
  let got=equivalent () term in
  expect got "let(r\"fee\",+(i4,v0))";
  let open_term=Let(Int 4,Let(Bool true,If(Var Z,Add(Var(S Z),Var(S(S Z))),Int 0))) in
  let got=equivalent (9,()) open_term in
  expect got "+(i4,v0)";
  expect (Optimizer.normalize(Add(Mul(Int 6,Int 7),Int(-5)))) "i37"
| "dead_binding" ->
  let term=Let(Add(Var Z,Int 4),Let(Read "kept",Add(Var Z,Var(S(S Z))))) in
  let got=equivalent (9,()) term in
  expect got "let(r\"kept\",+(v0,v1))";
  let used=Let(Add(Var Z,Int 4),Let(Read "kept",Add(Var Z,Var(S Z)))) in
  let got=equivalent (9,()) used in
  expect got "let(+(v0,i4),let(r\"kept\",+(v0,v1)))";
  let effect=Let(Read "unused",Let(Int 4,Add(Var Z,Int 0))) in
  let got=equivalent () effect in
  expect got "let(r\"unused\",i4)";
  let discarded=Let(If(Bool false,Read "never",Var Z),Var(S Z)) in
  expect (equivalent (9,()) discarded) "v0"
| "projection_effects" ->
  let cases = [
    Fst(Pair(Add(Int 2,Int 3),Read "audit"));
    Snd(Pair(Read "audit",Add(Int 2,Int 3)));
    Mul(Int 0,Add(Read "kept",Int 0));
    Mul(Add(Read "kept",Int 0),Int 0)] in
  List.iter (fun term ->
    let got=equivalent () term in
    assert(Measure.nodes got<Measure.nodes term)) cases;
  expect (Optimizer.normalize(Fst(Pair(Read "a",Mul(Int 0,Var Z))))) "r\"a\"";
  expect (Optimizer.normalize(Snd(Pair(Int 2,Read "b")))) "r\"b\"";
  expect (Optimizer.normalize(Mul(Int 0,Var Z))) "i0"
| "sharing" ->
  let term=Let(Add(Read "a",Read "b"),
    Pair(Add(Var Z,Int 0),Let(Read "c",Mul(Int 1,Var(S Z))))) in
  let got=equivalent () term in
  expect got "let(+(r\"a\",r\"b\"),p(v0,let(r\"c\",v1)))";
  assert(outcome () got (-1)=(Ok(15,15),["a";"b";"c"]));
  let pure_used=Let(Add(Var Z,Int 4),Add(Var Z,Var Z)) in
  expect (equivalent (7,()) pure_used) "let(+(v0,i4),+(v0,v0))"
| "generated_equivalence" ->
  let rng=Random.State.make [|937;2026;11|] in
  for i=0 to 1499 do
    let body=generated rng (i mod 6) in
    let term=Add(body,Int 0) in
    let got=equivalent () term in
    assert(Measure.nodes got<Measure.nodes term)
  done
| _ -> failwith "unknown check"
