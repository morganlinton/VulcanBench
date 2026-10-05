let () = let t=Graph.create ["x",3] in let s=Option.get(Graph.subscribe t "x") in Graph.stabilize t; assert(Graph.value s=3); assert(Graph.subscribe t "absent"=None)
