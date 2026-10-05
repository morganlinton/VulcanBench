let ok = function Ok()->() | Error e->failwith e
let sub t n=Option.get(Graph.subscribe t n)
let tx t sources formulas=ok(Graph.transaction t ~sources ~formulas)
let () = match Sys.argv.(1) with
| "source_observer" -> let t=Graph.create["x",3] in let s=sub t "x" in Graph.stabilize t; assert(Graph.value s=3); assert(Graph.subscribe t "missing"=None)
| "forward_atomic" -> let t=Graph.create["x",2] in tx t [] Expression.["out",Add(Ref "mid",Const 1);"mid",Add(Ref "x",Const 3)]; let s=sub t "out" in Graph.stabilize t; assert(Graph.value s=6); tx t ["x",8] []; assert(Graph.value s=6); Graph.stabilize t; assert(Graph.value s=12)
| "dynamic_switch" -> let t=Graph.create["flag",0;"a",2;"b",9] in tx t [] Expression.["selected",Select("flag",Ref "a",Ref "b");"out",Add(Ref "selected",Const 1)]; let s=sub t "out" in Graph.stabilize t; let before=Graph.additions t in tx t ["b",22] []; Graph.stabilize t; assert(Graph.value s=3); assert(Graph.additions t=before); tx t ["flag",1] []; Graph.stabilize t; assert(Graph.value s=23); tx t ["a",999] []; let before=Graph.additions t in Graph.stabilize t; assert(Graph.value s=23); assert(Graph.additions t=before)
| "atomic_dependency_reversal" ->
  let t=Graph.create["x",2] in
  tx t [] Expression.["a",Add(Ref "x",Const 1);"b",Add(Ref "a",Const 1)];
  let a=sub t "a" and b=sub t "b" in Graph.stabilize t;
  assert(Graph.value a=3 && Graph.value b=4);
  tx t [] Expression.["a",Add(Ref "b",Const 1);"b",Add(Ref "x",Const 1)];
  assert(Graph.value a=3 && Graph.value b=4); Graph.stabilize t;
  assert(Graph.value a=4 && Graph.value b=3);
  tx t [] Expression.["a",Add(Ref "x",Const 1);"b",Add(Ref "a",Const 1)];
  Graph.stabilize t; assert(Graph.value a=3 && Graph.value b=4)
| "repeated_dynamic_switch" ->
  let t=Graph.create["flag",0;"x",2;"y",7] in
  tx t [] Expression.["out",Add(Select("flag",Ref "x",Ref "y"),Const 1)];
  let s=sub t "out" in Graph.stabilize t;
  List.iter(fun flag -> tx t ["flag",flag] []; Graph.stabilize t;
    assert(Graph.value s=(if flag=0 then 3 else 8))) [1;0;1;0;1;0]
| "stable_subscription" -> let t=Graph.create["x",4] in tx t [] Expression.["out",Add(Ref "x",Const 1)]; let s=sub t "out" in Graph.stabilize t; tx t [] Expression.["out",Add(Ref "x",Const 8)]; assert(Graph.value s=5); Graph.stabilize t; assert(Graph.value s=12)
| "cutoff_laziness" -> let t=Graph.create["flag",0;"x",5;"y",5] in tx t [] Expression.["selected",Select("flag",Ref "x",Ref "y");"out",Add(Ref "selected",Const 1);"unused",Add(Ref "x",Ref "y")]; let s=sub t "out" in Graph.stabilize t; assert(Graph.additions t=1); let before=Graph.additions t in tx t ["flag",1] []; Graph.stabilize t; assert(Graph.value s=6); assert(Graph.additions t=before); tx t ["y",5] Expression.["out",Add(Ref "selected",Const 1)]; Graph.stabilize t; assert(Graph.additions t=before)
| "invalid_rollback" -> let t=Graph.create["x",2] in tx t [] Expression.["out",Add(Ref "x",Const 1)]; let s=sub t "out" in Graph.stabilize t; assert(Graph.transaction t ~sources:["x",9] ~formulas:Expression.["out",Ref "out"]=Error "cycle"); Graph.stabilize t; assert(Graph.value s=3); assert(Graph.transaction t ~sources:["x",8;"x",9] ~formulas:[]=Error "duplicate edit"); assert(Graph.transaction t ~sources:["x",8] ~formulas:Expression.["out",Ref "missing"]=Error "unknown reference"); Graph.stabilize t; assert(Graph.value s=3)
| _->failwith "unknown case"
