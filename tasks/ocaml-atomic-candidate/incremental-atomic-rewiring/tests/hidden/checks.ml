module F () = struct
  module I = Incremental.Make ()
  module N = I.Expert.Node
  module D = I.Expert.Dependency
  let ok = function Ok () -> () | Error s -> failwith ("unexpected rejection: " ^ s)
  let error expected = function
    | Error actual when actual = expected -> ()
    | Error s -> failwith ("expected " ^ expected ^ ", got " ^ s)
    | Ok () -> failwith ("expected rejection: " ^ expected)
  let inv () = I.State.invariant I.State.t
  let stabilize () = I.stabilize (); inv ()
  let make ?(offset=0) () =
    let deps = ref [] and calls = ref 0 in
    let node = N.create (fun () -> incr calls;
      offset + List.fold_left (fun acc d -> acc + D.value d) 0 !deps) in
    node, deps, calls
  let link (n, deps, _) ds = deps := ds; List.iter (N.add_dependency n) ds
  let replace (n, deps, _) ds = ok (N.apply_dependency_edits [N.Replace (n, ds)]); deps := ds
  let observe (n, _, _) = I.observe (N.watch n)
  let value = I.Observer.value_exn
end

let prospective_topology () =
  let module T = F () in let open T in
  I.State.set_max_height_allowed I.State.t 10000;
  let x = I.Var.create 3 in
  let a = make ~offset:1 () and b = make ~offset:10 () in
  let na,da,_ = a and nb,db,_ = b in
  let dx = D.create (I.Var.watch x) and ab = D.create (N.watch nb) and ba = D.create (N.watch na) in
  link a [dx]; link b [ba];
  let oa = observe a and ob = observe b in stabilize ();
  assert (value oa = 4 && value ob = 14);
  ok (N.apply_dependency_edits [N.Replace (na,[ab]); N.Replace (nb,[dx])]);
  da := [ab]; db := [dx]; inv ();
  assert (value oa = 4 && value ob = 14); stabilize ();
  assert (value oa = 14 && value ob = 13);
  (* Repeated reversals in both input orders, with reused detached edges. *)
  for i = 1 to 12 do
    if i mod 2 = 1 then (
      ok (N.apply_dependency_edits [N.Replace (nb,[ba]);N.Replace (na,[dx])]);
      da := [dx]; db := [ba])
    else (
      ok (N.apply_dependency_edits [N.Replace (nb,[dx]);N.Replace (na,[ab])]);
      da := [ab]; db := [dx]);
    I.Var.set x i; stabilize ();
    if i mod 2 = 1 then assert (value oa = i+1 && value ob = i+11)
    else assert (value oa = i+11 && value ob = i+10)
  done;
  (* Dormant cycles count too. *)
  let c = make () and d = make () in
  let nc,_,_ = c and nd,_,_ = d in
  link c [D.create (N.watch nd)];
  error "cycle" (N.apply_dependency_edits [N.Replace (nd,[D.create (N.watch nc)])]);
  error "cycle" (N.apply_dependency_edits [N.Replace (nc,[D.create (N.watch nc)])]); inv ()

let rejection_atomicity () =
  let module T = F () in let open T in
  let x = I.Var.create 2 in
  let a = make () and b = make ~offset:1 () in
  let na,_,ca = a and nb,_,cb = b in
  let dx = D.create (I.Var.watch x) in
  link a [dx]; link b [D.create (N.watch na)];
  let oa = observe a and ob = observe b in stabilize ();
  let initial = !ca,!cb in
  error "cycle" (N.apply_dependency_edits [N.Replace (na,[D.create (N.watch nb)])]);
  error "duplicate node" (N.apply_dependency_edits [N.Replace (na,[dx;dx]);N.Replace (na,[])]);
  error "duplicate dependency" (N.apply_dependency_edits [N.Replace (na,[dx]);N.Replace (nb,[dx])]);
  assert ((!ca,!cb)=initial && value oa=2 && value ob=3); inv ();
  stabilize (); assert ((!ca,!cb)=initial);
  I.Var.set x 7; stabilize (); assert (value oa=7 && value ob=8);
  (* Higher-precedence in-use error must win over a simultaneous cycle. *)
  let foreign = make () in let nf,_,_ = foreign in
  let taken = D.create (I.const 90) in link foreign [taken];
  error "dependency in use" (N.apply_dependency_edits [N.Replace (na,[taken;D.create (N.watch na)])]);
  replace a [D.create (I.const 4)]; stabilize (); assert (value oa=4 && value ob=5);
  replace foreign []; ignore nf; inv ()

let ownership_transfer () =
  let module T = F () in let open T in
  let x = I.Var.create 4 and y = I.Var.create 9 in
  let a = make () and b = make () in let na,da,_=a and nb,db,_=b in
  let dx = D.create (I.Var.watch x) and dy = D.create (I.Var.watch y) in
  link a [dx;dy];
  let oa=observe a and ob=observe b in stabilize ();
  (* Moving a non-last edge also challenges compact-array indices. *)
  ok (N.apply_dependency_edits [N.Replace (nb,[dx]);N.Replace (na,[dy])]); da := [dy]; db := [dx];
  stabilize (); assert (value oa=9 && value ob=4);
  I.Var.set x 12; stabilize (); assert (value oa=9 && value ob=12);
  replace b []; N.add_dependency na dx; da := [dy;dx]; stabilize ();
  assert (value oa=21 && value ob=0); inv ();
  (* Two different edge objects to one child are legal and both contribute. *)
  let dx2 = D.create (I.Var.watch x) in replace a [dx;dx2]; stabilize (); assert (value oa=24);
  (* Retained object still belongs to its parent after replacement. *)
  error "dependency in use" (N.apply_dependency_edits [N.Replace (nb,[dx])]); inv ()

let publication_callbacks () =
  let module T = F () in let open T in
  let x=I.Var.create 5 and y=I.Var.create 20 in
  let events = ref [] in
  let edge label incr = D.create incr ~on_change:(fun v -> events := !events @ [label,v]) in
  let a=make () in let na,_,calls=a in
  let old=edge "old" (I.Var.watch x) in link a [old];
  let oa=observe a in stabilize (); events := [];
  let fresh1=edge "new1" (I.Var.watch y) and fresh2=edge "new2" (I.Var.watch x) in
  let c = !calls in replace a [fresh1;fresh2];
  assert (!events=[] && !calls=c && value oa=5); inv ();
  I.Var.set y 22; stabilize ();
  assert (!events=["new1",22;"new2",5] && value oa=27 && !calls=c+1);
  events := []; I.Var.set x 8; stabilize ();
  assert (!events=["new2",8] && value oa=30);
  events := []; replace a [fresh2;fresh1]; assert (!events=[]); stabilize ();
  assert (!events=["new2",8;"new1",22]);
  ignore na

let necessity_and_noops () =
  let module T = F () in let open T in
  let transitions=ref [] and reentrant=ref [] in
  let a=make () in let na,_,ca=a in
  let lazy_calls=ref 0 in
  let lazy_node=N.create ~on_observability_change:(fun ~is_now_observable ->
    transitions := !transitions @ [is_now_observable];
    reentrant := N.apply_dependency_edits [] :: !reentrant)
    (fun () -> incr lazy_calls; 7) in
  let d=D.create (N.watch lazy_node) in
  replace a [d]; assert (!ca=0 && !lazy_calls=0 && !transitions=[]);
  let oa=observe a in stabilize (); assert (value oa=7);
  transitions := []; reentrant := [];
  let before = !ca,!lazy_calls in
  replace a [d]; ok (N.apply_dependency_edits []); stabilize ();
  assert ((!ca,!lazy_calls)=before && !transitions=[] && !reentrant=[]);
  replace a []; assert (!transitions=[false]);
  List.iter (error "busy") !reentrant;
  stabilize (); assert (value oa=0); inv ();
  replace a [d]; List.iter (error "busy") !reentrant;
  stabilize (); assert (value oa=7);
  let downstream_calls=ref 0 in
  let out=I.observe (I.map (N.watch na) ~f:(fun v -> incr downstream_calls; v)) in
  stabilize (); let c = !downstream_calls in
  replace a [D.create (I.const 7)]; stabilize ();
  assert (value out=7 && !downstream_calls=c);
  I.Observer.disallow_future_use oa; I.Observer.disallow_future_use out; stabilize ();
  let c = !ca in replace a [D.create (I.const 8)]; stabilize (); assert (!ca=c)

let mixed_graph () =
  let module T = F () in let open T in
  let x=I.Var.create 2 in
  let a=make () in let na,_,_=a in
  link a [D.create (I.Var.watch x)];
  let middle=I.map (N.watch na) ~f:succ in
  let output=I.map middle ~f:(fun n -> n*2) in let o=I.observe output in stabilize ();
  error "cycle" (N.apply_dependency_edits [N.Replace (na,[D.create middle])]);
  assert (value o=6); I.Var.set x 4; stabilize (); assert (value o=10);
  replace a [D.create (I.const 9)]; stabilize (); assert (value o=20);
  (* Invalid top-level expert targets and bind-scoped targets have explicit errors. *)
  let dead=make () in let nd,_,_=dead in N.invalidate nd;
  error "invalid node" (N.apply_dependency_edits [N.Replace (nd,[])]);
  let scoped=ref None in
  let trigger=I.Var.create 0 in
  let observer=I.observe (I.bind (I.Var.watch trigger) ~f:(fun _ ->
    let n=N.create (fun () -> 3) in scoped := Some n; N.watch n)) in
  stabilize ();
  let scoped=Option.get !scoped in
  error "invalid node" (N.apply_dependency_edits [N.Replace (na,[D.create (N.watch scoped)])]);
  I.Var.set trigger 1; stabilize ();
  error "invalid node" (N.apply_dependency_edits [N.Replace (scoped,[])]);
  ignore observer; inv ()

let busy_and_generic () =
  let module T = F () in let open T in
  ignore (I.Expert.do_one_step_of_stabilize ());
  error "busy" (N.apply_dependency_edits []);
  let rec finish () = match I.Expert.do_one_step_of_stabilize () with
    | I.Expert.Step_result.Done -> () | Keep_going -> finish () in finish ();
  let seen=ref [] in
  let v=I.Var.create 1 in
  let o=I.observe (I.map (I.Var.watch v) ~f:(fun v -> seen := N.apply_dependency_edits [] :: !seen; v)) in
  I.Observer.on_update_exn o ~f:(fun _ -> seen := N.apply_dependency_edits [] :: !seen);
  stabilize (); assert (List.length !seen>=2); List.iter (error "busy") !seen;
  ok (N.apply_dependency_edits []);
  let module S = (val Incremental.State.create ()) in
  let module G=Incremental in let module GN=G.Expert.Node in let module GD=G.Expert.Dependency in
  let deps=ref [] in
  let n=GN.create S.t (fun () -> List.fold_left (fun a d -> a+GD.value d) 0 !deps) in
  let string_deps=ref [] in
  let text=GN.create S.t (fun () -> string_of_int (List.fold_left (fun a d -> a+GD.value d) 0 !string_deps)) in
  let d=GD.create (G.const S.t 17) and e=GD.create (G.const S.t 19) in
  ok (GN.apply_dependency_edits S.t [GN.Replace (n,[d]);GN.Replace (text,[e])]);
  deps := [d]; string_deps := [e];
  let on=G.observe (GN.watch n) and ot=G.observe (GN.watch text) in
  G.stabilize S.t; assert (G.Observer.value_exn on=17 && G.Observer.value_exn ot="19");
  let module P=F () in
  let bad=P.I.observe (P.I.map (P.I.const 0) ~f:(fun _ -> failwith "poison")) in
  (try P.I.stabilize (); assert false with Failure _ -> ());
  P.error "busy" (P.N.apply_dependency_edits []); ignore bad

let ordinary () =
  let module T = F () in let open T in
  let x=I.Var.create 3 in let a=make () in let na,da,_=a in
  let d=D.create (I.Var.watch x) in link a [d];
  let o=observe a in stabilize (); assert (value o=3);
  I.Var.set x 9; stabilize (); assert (value o=9);
  (* Exercise the established removal rule from a child computation. *)
  let remover=I.map (I.Var.watch x) ~f:(fun v ->
    if v=10 then (N.remove_dependency na d; da := [])) in
  N.add_dependency na (D.create remover);
  I.Var.set x 10; stabilize (); assert (value o=0); inv ()

let () = match Sys.argv.(1) with
| "prospective_topology" -> prospective_topology ()
| "rejection_atomicity" -> rejection_atomicity ()
| "ownership_transfer" -> ownership_transfer ()
| "publication_callbacks" -> publication_callbacks ()
| "necessity_and_noops" -> necessity_and_noops ()
| "mixed_graph" -> mixed_graph ()
| "busy_and_generic" -> busy_and_generic ()
| "ordinary" -> ordinary ()
| _ -> failwith "unknown check"
