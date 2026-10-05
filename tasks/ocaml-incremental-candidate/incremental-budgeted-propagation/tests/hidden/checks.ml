let check b = if not b then failwith "contract mismatch"
let raises f = try f (); false with _ -> true
exception Stop of int ref

module Run (I : Incremental.S) = struct
  let count () = I.State.num_nodes_recomputed I.State.t
  let cycles () = I.State.num_stabilizes I.State.t
  let slice quota =
    let before = count () in
    let result = I.stabilize_with_budget ~max_recomputations:quota () in
    let used = count () - before in
    check (used >= 0 && used <= quota);
    (match result with `More -> check (used = quota && I.am_stabilizing ())
     | `Done -> check (not (I.am_stabilizing ())));
    result
  let drain quota =
    let rec loop n =
      check (n < 2000);
      match slice quota with `Done -> () | `More -> loop (n + 1)
    in loop 0
  let chain n x =
    let rec loop n x = if n = 0 then x else loop (n-1) (I.map x ~f:succ) in
    loop n x
end

let exact_quota () =
  let module I = Incremental.Make () in
  let module R = Run (I) in
  I.State.set_max_height_allowed I.State.t 512;
  let v = I.Var.create 10 in
  let o = I.observe (R.chain 220 (I.Var.watch v)) in
  let before = R.count () and cycle = R.cycles () in
  let quotas = [|1; 2; 7; 3|] in
  let rec loop n =
    check (n < 200);
    match R.slice quotas.(n mod 4) with `More -> check (R.cycles () = cycle); loop (n+1)
    | `Done -> ()
  in loop 0;
  check (R.count () - before = 221);
  check (R.cycles () = cycle+1 && I.Observer.value_exn o = 230);
  I.Var.set v 20; R.drain 1;
  check (I.Observer.value_exn o = 240);
  let empty_before = R.count () and empty_cycle = R.cycles () in
  check (R.slice 1 = `Done && R.count () = empty_before && R.cycles () = empty_cycle+1);
  let module J = Incremental.Make () in
  let module S = Run (J) in
  let x = J.Var.create 1 and y = J.Var.create 2 in
  let o = J.observe (J.map2 (J.Var.watch x) (J.Var.watch y) ~f:(+)) in
  S.drain 1; check (J.Observer.value_exn o = 3);
  J.Var.set x 5; check (S.slice 1 = `More); check (S.slice 1 = `Done);
  check (J.Observer.value_exn o = 7)

let dynamic_dependencies () =
  let module I = Incremental.Make () in
  let module R = Run (I) in
  let selector = I.Var.create 0 and left = I.Var.create 2 and right = I.Var.create 7 in
  let scoped = ref [] in
  let bound = I.bind (I.Var.watch selector) ~f:(fun i ->
    let n = R.chain (i mod 4 + 1) (if i mod 2 = 0 then I.Var.watch left else I.Var.watch right) in
    scoped := n :: !scoped; n) in
  let condition = I.map (I.Var.watch selector) ~f:(fun i -> i mod 3 = 0) in
  let conditional = I.if_ condition ~then_:bound ~else_:(I.Var.watch right) in
  let node_var = I.Var.create bound in
  let joined = I.join (I.Var.watch node_var) in
  let diamond = I.map2 conditional joined ~f:(+) in
  let observer = I.observe diamond in
  for i = 0 to 24 do
    I.Var.set selector i; I.Var.set left (i+2); I.Var.set right (i+7);
    I.Var.set node_var (if i mod 5 = 0 then I.Var.watch left else bound);
    R.drain (i mod 3 + 1);
    let b = (if i mod 2 = 0 then i+2 else i+7) + i mod 4 + 1 in
    let c = if i mod 3 = 0 then b else i+7 in
    let j = if i mod 5 = 0 then i+2 else b in
    check (I.Observer.value_exn observer = c+j);
    I.State.invariant I.State.t
  done;
  check (List.exists (fun n -> not (I.is_valid n)) !scoped);
  let calls = ref 0 in
  let v = I.Var.create 0 in
  let parity = I.map (I.Var.watch v) ~f:(fun i -> i mod 2) in
  I.set_cutoff parity I.Cutoff.poly_equal;
  let cut_observer = I.observe (I.map parity ~f:(fun n -> incr calls; n+10)) in
  R.drain 1; let old_calls = !calls in
  I.Var.set v 2; R.drain 1;
  check (!calls = old_calls && I.Observer.value_exn cut_observer = 10);
  let dependency = I.Expert.Dependency.create (I.Var.watch v) in
  let expert = I.Expert.Node.create (fun () -> I.Expert.Dependency.value dependency * 3) in
  I.Expert.Node.add_dependency expert dependency;
  let eo = I.observe (I.Expert.Node.watch expert) in
  R.drain 1; check (I.Observer.value_exn eo = 6);
  I.Var.set v 9; R.drain 1; check (I.Observer.value_exn eo = 27)

let publication_fence () =
  let module I = Incremental.Make () in
  let module R = Run (I) in
  let v = I.Var.create 4 and created = ref None and events = ref [] in
  let n = I.map (I.Var.watch v) ~f:(fun x ->
    if !created = None then created := Some (I.observe (I.const 99)); x+1) in
  let o = I.observe (R.chain 5 n) in
  I.Observer.on_update_exn o ~f:(fun _ -> events := I.Observer.value_exn o :: !events);
  let cycle = R.cycles () in
  check (R.slice 1 = `More);
  check (!events = [] && R.cycles () = cycle && raises (fun () -> ignore (I.Observer.value_exn o)));
  R.drain 1; check (!events = [10] && R.cycles () = cycle+1);
  let new_o = Option.get !created in
  check (raises (fun () -> ignore (I.Observer.value_exn new_o)));
  R.drain 1; check (I.Observer.value_exn new_o = 99 && !events = [10]);
  I.Var.set v 11;
  check (R.slice 1 = `More && !events = [10]);
  R.drain 2; check (!events = [17;10])

let deferred_writes () =
  let module I = Incremental.Make () in
  let module R = Run (I) in
  let v = I.Var.create 1 and scheduled = ref false in
  let n = I.map (I.Var.watch v) ~f:(fun x ->
    if not !scheduled then (scheduled := true; I.Var.set v 8; I.Var.set v 9); x) in
  let o = I.observe (R.chain 8 n) in
  check (R.slice 1 = `More);
  I.Var.set v 6; I.Var.set v 7;
  (* The callback runs later and becomes the final queued writer. *)
  R.drain 1; check (I.Observer.value_exn o = 9 && I.Var.value v = 9);
  R.drain 1; check (I.Observer.value_exn o = 17);
  I.Var.set v 12; check (R.slice 1 = `More);
  I.Var.set v 21; I.Var.set v 22;
  R.drain 1; check (I.Observer.value_exn o = 20 && I.Var.value v = 22);
  R.drain 1; check (I.Observer.value_exn o = 30)

let driver_interop () =
  let module I = Incremental.Make () in
  let module R = Run (I) in
  let v = I.Var.create 2 in
  let o = I.observe (R.chain 20 (I.Var.watch v)) in
  check (R.slice 1 = `More); I.stabilize ();
  check (I.Observer.value_exn o = 22 && R.cycles () = 1);
  I.Var.set v 9;
  check (I.Expert.do_one_step_of_stabilize () = I.Expert.Step_result.Keep_going);
  R.drain 2; check (I.Observer.value_exn o = 29 && R.cycles () = 2);
  I.Var.set v 4; check (R.slice 1 = `More);
  let rec step n = check (n < 100);
    match I.Expert.do_one_step_of_stabilize () with
    | I.Expert.Step_result.Done -> () | Keep_going -> step (n+1)
  in step 0; check (I.Observer.value_exn o = 24 && R.cycles () = 3);
  let module G = (val Incremental.State.create ()) in
  let module H = (val Incremental.State.create ()) in
  let x = Incremental.Var.create G.t 3 in
  let go = Incremental.observe (Incremental.map (Incremental.Var.watch x) ~f:succ) in
  let ho = Incremental.observe (Incremental.const H.t 18) in
  check (Incremental.stabilize_with_budget ~max_recomputations:1 G.t = `More);
  check (Incremental.stabilize_with_budget ~max_recomputations:1 H.t = `Done);
  check (Incremental.Observer.value_exn ho = 18);
  check (Incremental.stabilize_with_budget ~max_recomputations:1 G.t = `Done);
  check (Incremental.Observer.value_exn go = 4)

let reentrancy () =
  let module I = Incremental.Make () in
  let module R = Run (I) in
  let module J = Incremental.Make () in
  let jo = J.observe (J.const 31) in
  let rejections = ref 0 in
  let callback () =
    let before = R.count () in
    List.iter (fun f -> check (raises f); incr rejections;
      check (R.count () = before))
      [I.stabilize; (fun () -> ignore (I.Expert.do_one_step_of_stabilize ()));
       (fun () -> ignore (I.stabilize_with_budget ~max_recomputations:2 ()))];
    check (J.stabilize_with_budget ~max_recomputations:1 () = `Done);
    check (J.Observer.value_exn jo = 31)
  in
  let v = I.Var.create 1 in
  let o = I.observe (I.map (I.Var.watch v) ~f:(fun x -> callback (); x+1)) in
  I.Observer.on_update_exn o ~f:(fun _ -> callback ());
  R.drain 1; check (!rejections = 6 && I.Observer.value_exn o = 2);
  I.Var.set v 2; I.stabilize ();
  check (!rejections = 12 && I.Observer.value_exn o = 3);
  I.Var.set v 3;
  let rec step n = check (n < 100);
    match I.Expert.do_one_step_of_stabilize () with Done -> () | Keep_going -> step (n+1)
  in step 0; check (!rejections = 18 && I.Observer.value_exn o = 4)

let poison_and_validation () =
  let module I = Incremental.Make () in
  let module R = Run (I) in
  let v = I.Var.create 3 in
  let failure = Stop (ref 9) and calls = ref 0 in
  let o = I.observe (I.map (I.Var.watch v) ~f:(fun _ -> incr calls; raise failure)) in
  let invalid quota =
    let before = R.count () and cycle = R.cycles () in
    let caught = try ignore (I.stabilize_with_budget ~max_recomputations:quota ()); false
      with Invalid_argument _ -> true | _ -> false in
    check (caught && R.count () = before && R.cycles () = cycle)
  in invalid 0; invalid (-5);
  check (R.slice 1 = `More); invalid 0;
  let caught = try ignore (R.slice 1); false with ex -> ex == failure in
  check (caught && !calls = 1);
  let before = R.count () in
  List.iter (fun f -> check (raises f); check (R.count () = before && !calls = 1))
    [I.stabilize; (fun () -> ignore (I.Expert.do_one_step_of_stabilize ()));
     (fun () -> ignore (I.stabilize_with_budget ~max_recomputations:10 ()))];
  invalid 0;
  check (raises (fun () -> ignore (I.Observer.value_exn o)));
  let stored = try I.stabilize (); None with Core.Exn.Reraised (_,ex) -> Some ex | _ -> None in
  check (match stored with Some ex -> ex == failure | None -> false)

let ordinary () =
  let module I = Incremental.Make () in
  let module R = Run (I) in
  let v = I.Var.create 2 in
  let o = I.observe (R.chain 12 (I.Var.watch v)) in
  I.stabilize (); check (I.Observer.value_exn o = 14);
  let w = I.Var.create 2 and y = I.Var.create 9 in
  let o2 = I.observe (I.map2 (I.Var.watch w) (I.Var.watch y) ~f:(+)) in
  I.stabilize (); I.Var.set v 4; I.stabilize (); I.Var.set w 4; I.stabilize ();
  check (I.Observer.value_exn o2 = 13 && I.Observer.value_exn o = 16);
  check (I.State.num_nodes_recomputed_directly_because_one_child I.State.t > 0);
  check (I.State.num_nodes_recomputed_directly_because_min_height I.State.t > 0)

let () =
  Printexc.record_backtrace true;
  match Sys.argv.(1) with
  | "exact_quota" -> exact_quota ()
  | "dynamic_dependencies" -> dynamic_dependencies ()
  | "publication_fence" -> publication_fence ()
  | "deferred_writes" -> deferred_writes ()
  | "driver_interop" -> driver_interop ()
  | "reentrancy" -> reentrancy ()
  | "poison_and_validation" -> poison_and_validation ()
  | "ordinary" -> ordinary ()
  | _ -> failwith "unknown group"
