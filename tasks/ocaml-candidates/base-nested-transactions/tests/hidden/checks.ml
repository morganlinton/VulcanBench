module H = Base.Hashtbl
exception Stop of int 
let check b = if not b then failwith "contract mismatch"
let abort t f =
  let ex = Stop 17 in
  try H.with_transaction t ~f:(fun () -> f (); raise ex)
  with | e -> check (e == ex)
let make () =
  let t = H.create (module Base.Int) in
  List.iter (fun (k, v) -> H.set t ~key:k ~data:v)
    [(1, 10); (2, 20); (3, 30)];
  t
let sorted t = List.sort compare (H.to_alist t)
let valid t = H.invariant ignore ignore t
let rollback_mutators () =
  let actions =
    [(fun t -> H.set t ~key:1 ~data:99);
    (fun t -> ignore (H.add t ~key:4 ~data:40));
    (fun t -> H.remove t 2);
    (fun t -> ignore (H.find_and_remove t 1));
    (fun t -> H.clear t);
    (fun t -> H.change t 1 ~f:(fun _ -> None));
    (fun t -> H.update t 2 ~f:(fun _ -> 99));
    (fun t -> ignore (H.update_and_return t 5 ~f:(fun _ -> 88)));
    (fun t -> ignore (H.find_or_add t 9 ~default:(fun () -> 90)));
    (fun t -> ignore (H.findi_or_add t 8 ~default:(fun k -> k * 10)));
    (fun t -> H.incr t 1);
    (fun t -> H.decr ~remove_if_zero:true ~by:20 t 2);
    (fun t -> H.map_inplace t ~f:succ);
    (fun t -> H.mapi_inplace t ~f:(fun ~key ~data -> key + data));
    (fun t -> H.filteri_inplace t ~f:(fun ~key ~data:_ -> key = 1));
    (fun t -> H.filter_keys_inplace t ~f:(fun k -> k = 1));
    (fun t ->
       H.filter_map_inplace t
         ~f:(fun d -> if d = 10 then None else Some (d + 2)));
    (fun t ->
       let src = make () in
       H.merge_into ~src ~dst:t
         ~f:(fun ~key:_ _ _ -> Base.Hashtbl.Merge_into_action.Set_to 99))] in
  List.iter
    (fun f ->
       let t = make () in
       let before = sorted t in
       abort t (fun () -> f t); check ((sorted t) = before); valid t) actions;
  (let t = H.create (module Base.Int) in
   H.add_multi t ~key:1 ~data:3;
   (let before = sorted t in
    abort t (fun () -> H.add_multi t ~key:1 ~data:5; H.remove_multi t 1);
    check ((sorted t) = before)))
let nested_savepoints () =
  let t = make () in
  let before = sorted t in
  abort t
    (fun () ->
       H.set t ~key:1 ~data:50;
       H.with_transaction t ~f:(fun () -> H.set t ~key:2 ~data:60));
  check ((sorted t) = before);
  H.with_transaction t
    ~f:(fun () ->
          H.set t ~key:1 ~data:70;
          abort t (fun () -> H.remove t 1; H.set t ~key:2 ~data:80);
          check (((H.find_exn t 1) = 70) && ((H.find_exn t 2) = 20));
          H.set t ~key:3 ~data:90);
  check ((sorted t) = [(1, 70); (2, 20); (3, 90)]);
  valid t
let callback_errors () =
  let t = make () in
  let before = sorted t
  and calls = ref 0 in
  (try
     H.with_transaction t
       ~f:(fun () ->
             H.mapi_inplace t
               ~f:(fun ~key:_ ~data ->
                     incr calls;
                     if (!calls) = 3 then raise (Stop 3);
                     data + 1))
   with | Stop 3 -> ());
  check ((sorted t) = before);
  (let ran = ref false in
   H.iter t
     ~f:(fun _ ->
           try H.with_transaction t ~f:(fun () -> ran := true) with | _ -> ());
   check (not (!ran));
   H.set t ~key:4 ~data:40;
   valid t)
module Key =
  struct
    type t = {
      id: int ;
      label: string }
    let sexp_of_t k = Base.Sexp.Atom (k.label)
    let compare a b = compare a.id b.id
    let hash _ = 0
  end
let canonical_keys () =
  let t = H.create (module Key)
  and original = { Key.id = 1; label = "original" }
  and replacement = { Key.id = 1; label = "other" } in
  let payload = ref 11 in
  H.set t ~key:original ~data:payload;
  abort t
    (fun () ->
       H.remove t replacement; H.set t ~key:replacement ~data:(ref 99));
  (match H.to_alist t with
   | (key, data)::[] -> check ((key == original) && (data == payload))
   | _ -> check false)
let resize_clear_copy () =
  let t = make () in
  let before = sorted t
  and copied = ref None in
  abort t
    (fun () ->
       for k = 4 to 1000 do H.set t ~key:k ~data:k done;
       H.clear t;
       H.set t ~key:8 ~data:88;
       copied := (Some (H.copy t)));
  check ((sorted t) = before);
  valid t;
  (let c = Option.get (!copied) in
   check ((sorted c) = [(8, 88)]);
   H.set c ~key:9 ~data:99;
   check (not (H.mem t 9)))
let sparse_journal () =
  let calls = ref 0 in
  let module K =
    struct
      type t = int
      let sexp_of_t = Base.Int.sexp_of_t
      let compare a b = incr calls; compare a b
      let hash k = incr calls; k
    end in
    let t = H.create ~size:131072 (module K) in
    for k = 0 to 49999 do H.set t ~key:k ~data:k done;
    calls := 0;
    Gc.full_major ();
    (let start = Gc.allocated_bytes () in
     abort t
       (fun () ->
          H.set t ~key:1 ~data:99;
          H.with_transaction t ~f:(fun () -> H.set t ~key:2 ~data:88));
     (let bytes = (Gc.allocated_bytes ()) -. start in
      let n = !calls in
      check (((H.find_exn t 1) = 1) && ((H.find_exn t 2) = 2));
      check ((bytes < 262144.) && (n <= 1024));
      Printf.printf "allocation=%.0f calls=%d\n" bytes n))
let ordinary () =
  let t = make () in
  check ((H.with_transaction t ~f:(fun () -> 17)) = 17);
  H.set t ~key:1 ~data:44;
  check ((H.find_exn t 1) = 44);
  valid t
let () =
  match Sys.argv.(1) with
  | "rollback_mutators" -> rollback_mutators ()
  | "nested_savepoints" -> nested_savepoints ()
  | "callback_errors" -> callback_errors ()
  | "canonical_keys" -> canonical_keys ()
  | "resize_clear_copy" -> resize_clear_copy ()
  | "sparse_journal" -> sparse_journal ()
  | "ordinary" -> ordinary ()
  | _ -> assert false
