open Async_kernel
let check b = if not b then failwith "contract mismatch"
let pump () =
  Async_kernel_scheduler.Expert.run_cycles_until_no_jobs_remain ()
type work =
  {
  key: string ;
  attempt: int ;
  gate: (string, string) result Ivar.t }
type append = {
  entry: Journal.entry ;
  gate: (unit, string) result Ivar.t }
let policy ?(slots= 2) ?(capacity= 8) ?(timeout= 5) ?(retry= 2) ?(attempts=
  2) () =
  Policy.create ~slots ~capacity ~timeout ~retry_delay:retry ~attempts
let make ?(policy= policy ()) () =
  let clock = Clock.create ()
  and works = ref []
  and appends = ref [] in
  let worker ~key ~payload:_ ~attempt =
    let w = { key; attempt; gate = (Ivar.create ()) } in
    works := ((!works) @ [w]); Ivar.read w.gate in
  let journal entry =
    let a = { entry; gate = (Ivar.create ()) } in
    appends := ((!appends) @ [a]); Ivar.read a.gate in
  let engine = Engine.create ~clock ~policy ~worker ~journal in
  (clock, engine, works, appends)
let submit t key = Option.get (Engine.submit t ~key ~payload:key)
let finish (works : work list ref) n result =
  Ivar.fill_exn (List.nth (!works) n).gate result; pump ()
let ack (appends : append list ref) n result =
  Ivar.fill_exn (List.nth (!appends) n).gate result; pump ()
let peek t = Deferred.peek (Ticket.result t)
let committed t v = check ((peek t) = (Some (Outcome.Committed v)))
let keyed_dispatch () =
  let (_, t, w, a) = make ~policy:(policy ~slots:2 ~capacity:4 ()) () in
  let x = submit t "x"
  and y = submit t "x"
  and z = submit t "z"
  and q = submit t "q" in
  check ((List.map (fun w -> w.key) (!w)) = ["x"; "z"]);
  check ((Engine.submit t ~key:"full" ~payload:"v") = None);
  finish w 1 (Ok "Z");
  check ((List.map (fun w -> w.key) (!w)) = ["x"; "z"; "q"]);
  check (((!a) = []) && ((peek z) = None));
  finish w 0 (Ok "X");
  check ((List.map (fun w -> w.key) (!w)) = ["x"; "z"; "q"; "x"]);
  check ((Engine.pending t) = 4);
  finish w 3 (Ok "Y");
  finish w 2 (Ok "Q");
  for i = 0 to 3 do ack a i (Ok ()) done;
  committed x (Succeeded "X");
  committed y (Succeeded "Y");
  committed z (Succeeded "Z");
  committed q (Succeeded "Q")
let ordered_durability () =
  let (_, t, w, a) = make ~policy:(policy ~slots:3 ()) () in
  let x = submit t "x"
  and y = submit t "y"
  and z = submit t "z" in
  finish w 2 (Ok "Z");
  finish w 1 (Ok "Y");
  check ((!a) = []);
  finish w 0 (Ok "X");
  check (((List.length (!a)) = 1) && ((peek x) = None));
  Ticket.cancel x;
  ack a 0 (Ok ());
  committed x (Succeeded "X");
  check ((List.map (fun a -> (a.entry).Journal.seq) (!a)) = [0; 1]);
  ack a 1 (Ok ());
  ack a 2 (Ok ());
  committed y (Succeeded "Y");
  committed z (Succeeded "Z");
  check ((Engine.pending t) = 0)
let retry_ownership () =
  let (clock, t, w, a) = make ~policy:(policy ~slots:1 ()) () in
  let x = submit t "x"
  and y = submit t "x"
  and z = submit t "z" in
  finish w 0 (Error "first");
  check ((List.map (fun w -> w.key) (!w)) = ["x"; "z"]);
  Clock.advance clock 2;
  pump ();
  check ((List.length (!w)) = 2);
  finish w 1 (Ok "Z");
  check ((List.map (fun w -> w.key) (!w)) = ["x"; "z"; "x"]);
  check ((List.nth (!w) 2).attempt = 2);
  finish w 2 (Ok "X");
  finish w 3 (Ok "Y");
  for i = 0 to 2 do ack a i (Ok ()) done;
  committed x (Succeeded "X");
  committed y (Succeeded "Y");
  committed z (Succeeded "Z");
  check ((Clock.pending clock) = 0)
let timeout_fences () =
  let (clock, t, w, a) = make ~policy:(policy ~slots:1 ()) () in
  let x = submit t "x"
  and z = submit t "z" in
  Clock.advance clock 5;
  pump ();
  check ((List.length (!w)) = 2);
  finish w 0 (Ok "late");
  check (((Engine.in_flight t) = 1) && ((!a) = []));
  Clock.advance clock 7;
  pump ();
  finish w 1 (Ok "Z");
  check ((List.length (!w)) = 3);
  Clock.advance clock 12;
  pump ();
  ack a 0 (Ok ());
  ack a 1 (Ok ());
  committed x (Failed "timeout");
  committed z (Succeeded "Z");
  finish w 2 (Ok "later");
  check ((Clock.pending clock) = 0);
  (let (clock, t, w, a) = make ~policy:(policy ~attempts:1 ()) () in
   let x = submit t "x" in
   Ivar.fill_exn (List.nth (!w) 0).gate (Ok "filled");
   Clock.advance clock 5;
   pump ();
   ack a 0 (Ok ());
   committed x (Failed "timeout"))
let cancel_commitpoint () =
  let (clock, t, w, a) = make ~policy:(policy ~slots:1 ()) () in
  let x = submit t "x"
  and y = submit t "y" in
  Ticket.cancel y;
  Ticket.cancel y;
  check ((peek y) = None);
  Ticket.cancel x;
  pump ();
  check (((Engine.in_flight t) = 0) && ((Clock.pending clock) = 0));
  ack a 0 (Ok ());
  ack a 1 (Ok ());
  committed x Cancelled;
  committed y Cancelled;
  finish w 0 (Ok "late");
  (let (clock, t, w, a) = make ~policy:(policy ~slots:1 ()) () in
   let x = submit t "x" in
   finish w 0 (Error "retry");
   Ticket.cancel x;
   pump ();
   Clock.advance clock 20;
   pump ();
   check ((List.length (!w)) = 1);
   ack a 0 (Ok ());
   committed x Cancelled;
   (let (_, t, w, a) = make () in
    let x = submit t "x" in
    finish w 0 (Ok "good");
    Ticket.cancel x;
    ack a 0 (Ok ());
    committed x (Succeeded "good")))
let close_abort () =
  let (clock, t, w, a) = make ~policy:(policy ~slots:1 ()) () in
  let x = submit t "x"
  and y = submit t "y" in
  let closed = Engine.close t in
  check
    (((Deferred.peek closed) = None) &&
       ((Engine.submit t ~key:"z" ~payload:"z") = None));
  finish w 0 (Ok "X");
  finish w 1 (Ok "Y");
  ack a 0 (Ok ());
  check ((Deferred.peek closed) = None);
  ack a 1 (Ok ());
  check ((Deferred.peek closed) = (Some ()));
  committed x (Succeeded "X");
  committed y (Succeeded "Y");
  check ((Clock.pending clock) = 0);
  (let (clock, t, w, a) = make () in
   let x = submit t "x"
   and y = submit t "y" in
   finish w 0 (Ok "X");
   (let closed = Engine.close t in
    Engine.abort t;
    check
      (((peek x) = (Some Outcome.Aborted)) &&
         ((peek y) = (Some Outcome.Aborted)));
    check (((Deferred.peek closed) = None) && ((Clock.pending clock) = 0));
    finish w 1 (Ok "late");
    ack a 0 (Ok ());
    check (((Deferred.peek closed) = (Some ())) && ((List.length (!a)) = 1));
    Engine.abort t;
    ignore (Engine.close t)))
let reentrant_callbacks () =
  let clock = Clock.create ()
  and engine = ref None
  and journal_calls = ref 0 in
  let worker ~key:_ ~payload:_ ~attempt:_ =
    Engine.abort (Option.get (!engine)); Deferred.return (Ok "ignored") in
  let journal _ = incr journal_calls; Deferred.return (Ok ()) in
  let t = Engine.create ~clock ~policy:(policy ()) ~worker ~journal in
  engine := (Some t);
  (let x = submit t "x" in
   pump ();
   check
     (((peek x) = (Some Outcome.Aborted)) &&
        (((!journal_calls) = 0) &&
           (((Engine.in_flight t) = 0) && ((Clock.pending clock) = 0))));
   (let clock = Clock.create ()
    and engine = ref None
    and gate = Ivar.create () in
    let worker ~key:_ ~payload ~attempt:_ = Deferred.return (Ok payload) in
    let journal _ = Engine.abort (Option.get (!engine)); Ivar.read gate in
    let t = Engine.create ~clock ~policy:(policy ()) ~worker ~journal in
    engine := (Some t);
    (let x = submit t "x" in
     pump ();
     (let closed = Engine.close t in
      check
        (((peek x) = (Some Outcome.Aborted)) &&
           ((Deferred.peek closed) = None));
      Ivar.fill_exn gate (Ok ());
      pump ();
      check ((Deferred.peek closed) = (Some ()));
      (let clock = Clock.create ()
       and engine = ref None
       and second = ref None in
       let worker ~key ~payload ~attempt:_ =
         if key = "first"
         then
           (second :=
              (Engine.submit (Option.get (!engine)) ~key:"second"
                 ~payload:"two");
            ignore (Engine.close (Option.get (!engine))));
         Deferred.return (Ok payload) in
       let journal _ = Deferred.return (Ok ()) in
       let t = Engine.create ~clock ~policy:(policy ()) ~worker ~journal in
       engine := (Some t);
       (let first = Option.get (Engine.submit t ~key:"first" ~payload:"one") in
        pump ();
        committed first (Succeeded "one");
        committed (Option.get (!second)) (Succeeded "two");
        check ((Deferred.peek (Engine.close t)) = (Some ()));
        (let clock = Clock.create ()
         and engine = ref None
         and victim = ref None in
         let worker ~key:_ ~payload ~attempt:_ = Deferred.return (Ok payload) in
         let journal entry =
           if entry.Journal.seq = 0
           then
             (Ticket.cancel (Option.get (!victim));
              ignore (Engine.close (Option.get (!engine))));
           Deferred.return (Ok ()) in
         let t = Engine.create ~clock ~policy:(policy ()) ~worker ~journal in
         engine := (Some t);
         (let first = submit t "a" in
          victim := (Some (submit t "b"));
          pump ();
          committed first (Succeeded "a");
          committed (Option.get (!victim)) Cancelled))))))))
let monitor_failures () =
  let clock = Clock.create () in
  let worker ~key:_ ~payload:_ ~attempt:_ = failwith "worker" in
  let journal _ = Deferred.return (Ok ()) in
  let t =
    Engine.create ~clock ~policy:(policy ~attempts:1 ()) ~worker ~journal in
  let x = submit t "x" in
  pump ();
  committed x (Failed "Failure(\"worker\")");
  check ((Clock.pending clock) = 0);
  (let clock = Clock.create () in
   let worker ~key:_ ~payload:_ ~attempt:_ =
     let monitor = Monitor.current () in
     Monitor.send_exn monitor (Failure "async worker"); Deferred.never () in
   let t =
     Engine.create ~clock ~policy:(policy ~attempts:1 ()) ~worker ~journal in
   let x = submit t "x" in
   pump ();
   committed x (Failed "Failure(\"async worker\")");
   check ((Clock.pending clock) = 0);
   (let (clock, t, w, a) = make () in
    let x = submit t "x"
    and y = submit t "y" in
    finish w 0 (Ok "X");
    ack a 0 (Error "disk");
    check
      (((peek x) = (Some (Outcome.Journal_failed "disk"))) &&
         ((peek y) = (Some (Outcome.Journal_failed "disk"))));
    finish w 1 (Ok "late");
    check (((List.length (!a)) = 1) && ((Clock.pending clock) = 0));
    List.iter
      (fun asynchronous ->
         let clock = Clock.create () in
         let worker ~key:_ ~payload ~attempt:_ = Deferred.return (Ok payload) in
         let journal _ =
           if asynchronous
           then
             (Monitor.send_exn (Monitor.current ()) (Failure "journal");
              Deferred.never ())
           else failwith "journal" in
         let t = Engine.create ~clock ~policy:(policy ()) ~worker ~journal in
         let x = submit t "x"
         and y = submit t "y" in
         pump ();
         check
           (((peek x) =
               (Some
                  (Outcome.Journal_failed
                     (Printexc.to_string (Failure "journal")))))
              && ((peek y) = (peek x)));
         check
           (((Clock.pending clock) = 0) &&
              ((Deferred.peek (Engine.close t)) = (Some ())))) [false; true]))
let seeded_delivery () =
  let (_, t, w, a) =
    make ~policy:(policy ~slots:1 ~attempts:1 ~capacity:64 ()) () in
  let rng = Random.State.make [|913;41|] in
  let tickets =
    List.init 50
      (fun i ->
         let key = string_of_int (Random.State.int rng 5) in
         (i, (submit t key))) in
  for i = 0 to 49 do (finish w i (Ok (string_of_int i)); ack a i (Ok ()))
  done;
  List.iter (fun (i, t) -> committed t (Succeeded (string_of_int i))) tickets;
  check
    ((List.map (fun x -> (x.entry).Journal.seq) (!a)) = (List.init 50 Fun.id));
  check ((Deferred.peek (Engine.close t)) = (Some ()))
let ordinary () =
  let (clock, t, _, _) = make () in
  check
    (((Engine.pending t) = 0) &&
       (((Engine.in_flight t) = 0) && ((Clock.pending clock) = 0)));
  check ((Deferred.peek (Engine.close t)) = (Some ()));
  check ((Engine.submit t ~key:"x" ~payload:"v") = None);
  check
    (try ignore (policy ~slots:0 ()); false with | Invalid_argument _ -> true)
let () =
  match Stdlib.Sys.argv.(1) with
  | "keyed_dispatch" -> keyed_dispatch ()
  | "ordered_durability" -> ordered_durability ()
  | "retry_ownership" -> retry_ownership ()
  | "timeout_fences" -> timeout_fences ()
  | "cancel_commitpoint" -> cancel_commitpoint ()
  | "close_abort" -> close_abort ()
  | "reentrant_callbacks" -> reentrant_callbacks ()
  | "monitor_failures" -> monitor_failures ()
  | "seeded_delivery" -> seeded_delivery ()
  | "ordinary" -> ordinary ()
  | _ -> assert false
