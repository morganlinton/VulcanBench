open Core
open Core.Poly
open Async_kernel

let pump () = Async_kernel_scheduler.Expert.run_cycles_until_no_jobs_remain ()
let seconds = Time_ns.Span.of_int_sec
let at n = Time_ns.add Time_ns.epoch (seconds n)
let policy ?(ttl=3) ?(timeout=10) ?(retry=2) ?(attempts=3) () =
  Policy.create ~ttl:(seconds ttl) ~timeout:(seconds timeout)
    ~retry_delay:(seconds retry) ~max_attempts:attempts
let advance source n =
  let done_=Time_source.advance_by_alarms source ~to_:(at n) in
  pump();assert(Deferred.peek done_=Some())
let peek request = Deferred.peek(Request.result request)
type call = { key:string; attempt:int; gate:(int,string) Result.t Ivar.t }
let make ?(policy=policy()) () =
  let source=Time_source.create ~now:Time_ns.epoch () in
  let calls=ref [] in
  let worker key ~attempt =
    let call={key;attempt;gate=Ivar.create()} in
    calls:= !calls @ [call];Ivar.read call.gate
  in
  let cache=Cache.create ~source:(Time_source.read_only source) ~policy ~worker in
  let finish index result = Ivar.fill_exn (List.nth_exn !calls index).gate result;pump() in
  source,cache,calls,finish
let pending_source source = Option.is_some(Time_source.next_alarm_fires_at source)
let no_alarms source cache =
  assert(Cache.pending_alarms cache=0);assert(not(pending_source source))

let () = match Stdlib.Sys.argv.(1) with
| "empty_policy_request" ->
  let source,cache,_,_=make() in
  assert(Cache.cached cache=0);assert(Cache.in_flight cache=0);no_alarms source cache;
  let request=Request.create() in
  let hooks=ref 0 in Request.on_cancel request (fun () -> incr hooks);
  Request.cancel request;Request.cancel request;Request.resolve request (Request.Value 2);
  assert(peek request=Some Request.Cancelled);assert(!hooks=1);
  (try ignore(policy ~timeout:0 ());assert false with Invalid_argument _ -> ());
  (try ignore(policy ~retry:0 ());assert false with Invalid_argument _ -> ());
  (try ignore(policy ~ttl:(-1) ());assert false with Invalid_argument _ -> ());
  (try ignore(policy ~attempts:0 ());assert false with Invalid_argument _ -> ());
  Cache.close cache;Cache.close cache;Cache.invalidate cache "x";
  assert(peek(Cache.get cache "x")=Some Request.Closed)
| "coalescing_ttl" ->
  let source,cache,calls,finish=make() in
  let a=Cache.get cache "x" and b=Cache.get cache "x" in
  assert(List.length !calls=1);assert(Cache.in_flight cache=1);
  let c=Cache.get cache "y" in assert(List.length !calls=2);
  assert(List.map !calls ~f:(fun call -> call.key)=["x";"y"]);
  finish 0 (Ok 11);assert(peek a=Some(Request.Value 11));assert(peek b=peek a);
  assert(peek c=None);assert(peek(Cache.get cache "x")=peek a);
  advance source 2;assert(peek(Cache.get cache "x")=peek a);
  advance source 3;assert(Cache.cached cache=0);
  let d=Cache.get cache "x" in assert(peek d=None);assert(List.length !calls=3);
  finish 2 (Ok 33);finish 1 (Ok 22);
  assert(peek d=Some(Request.Value 33));assert(peek c=Some(Request.Value 22));
  no_alarms source cache;
  let source,cache,calls,finish=make ~policy:(policy ~ttl:0 ()) () in
  let a=Cache.get cache "x" in finish 0 (Ok 1);assert(peek a=Some(Request.Value 1));
  assert(Cache.cached cache=0);ignore(Cache.get cache "x");assert(List.length !calls=2);
  Cache.close cache;no_alarms source cache
| "generation_invalidation" ->
  let source,cache,calls,finish=make() in
  let old=Cache.get cache "x" in Cache.invalidate cache "x";
  let middle=Cache.get cache "x" in Cache.invalidate cache "x";
  let fresh=Cache.get cache "x" and joined=Cache.get cache "x" in
  assert(List.length !calls=3);assert(Cache.in_flight cache=3);
  finish 2 (Ok 30);finish 0 (Ok 10);finish 1 (Ok 20);
  assert(peek old=Some(Request.Value 10));assert(peek middle=Some(Request.Value 20));
  assert(peek fresh=Some(Request.Value 30));assert(peek joined=peek fresh);
  assert(peek(Cache.get cache "x")=peek fresh);assert(Cache.cached cache=1);
  assert(Cache.in_flight cache=0);no_alarms source cache;
  Cache.invalidate cache "x";let older=Cache.get cache "x" in
  Cache.invalidate cache "x";let newest=Cache.get cache "x" in
  finish 3 (Ok 40);assert(peek newest=None);assert(Cache.in_flight cache=1);
  let other=Cache.get cache "x" in assert(List.length !calls=5);
  finish 4 (Ok 50);assert(peek older=Some(Request.Value 40));assert(peek other=peek newest)
| "subscriber_cancellation" ->
  let source,cache,calls,finish=make() in
  let a=Cache.get cache "x" and b=Cache.get cache "x" in
  Request.cancel a;Request.cancel a;assert(peek a=Some Request.Cancelled);
  assert(Cache.in_flight cache=1);finish 0 (Ok 2);assert(peek b=Some(Request.Value 2));
  Request.cancel b;assert(peek b=Some(Request.Value 2));
  Cache.invalidate cache "x";
  let c=Cache.get cache "x" in Request.cancel c;
  assert(Cache.in_flight cache=0);no_alarms source cache;
  let d=Cache.get cache "x" in finish 1 (Ok 99);
  assert(peek d=None);assert(Cache.cached cache=0);assert(List.length !calls=3);
  finish 2 (Ok 3);assert(peek d=Some(Request.Value 3));
  Cache.invalidate cache "x";let detached=Cache.get cache "x" in
  Cache.invalidate cache "x";let current=Cache.get cache "x" in
  Request.cancel detached;assert(Cache.in_flight cache=1);
  finish 3 (Error "late error");advance source 6;
  assert(List.length !calls=5);assert(peek current=None);
  Request.cancel current;no_alarms source cache
| "retry_budget" ->
  let source,cache,calls,finish=make() in
  let a=Cache.get cache "x" in finish 0 (Error "first");
  assert(Cache.pending_alarms cache=2);advance source 1;
  assert(List.length !calls=1);let b=Cache.get cache "x" in
  advance source 2;assert(List.length !calls=2);assert((List.nth_exn !calls 1).attempt=2);
  finish 1 (Error "second");advance source 4;assert(List.length !calls=3);
  assert((List.nth_exn !calls 2).attempt=3);finish 2 (Error "last");
  assert(peek a=Some(Request.Failed "last"));assert(peek b=peek a);
  assert(Cache.cached cache=0);no_alarms source cache;
  ignore(Cache.get cache "x");assert((List.nth_exn !calls 3).attempt=1);
  Cache.close cache;no_alarms source cache
| "deadline_dominance" ->
  let source,cache,calls,finish=make ~policy:(policy ~timeout:5 ~retry:2 ()) () in
  let a=Cache.get cache "x" in finish 0 (Error "first");advance source 2;
  finish 1 (Error "second");advance source 4;
  assert(List.length !calls=3);advance source 5;
  assert(peek a=Some Request.Timed_out);finish 2 (Ok 99);
  assert(Cache.cached cache=0);no_alarms source cache;
  let source,cache,calls,finish=make ~policy:(policy ~timeout:2 ~retry:2 ()) () in
  let a=Cache.get cache "x" in finish 0 (Error "retry at deadline");
  advance source 2;assert(List.length !calls=1);assert(peek a=Some Request.Timed_out);
  no_alarms source cache;
  let source,cache,calls,finish=make ~policy:(policy ~timeout:2 ()) () in
  let a=Cache.get cache "x" in
  Time_source.advance_directly source ~to_:(at 2);
  let b=Cache.get cache "x" in assert(peek a=Some Request.Timed_out);
  assert(List.length !calls=2);finish 0 (Ok 99);assert(peek b=None);
  finish 1 (Ok 7);assert(peek b=Some(Request.Value 7));no_alarms source cache
| "close_detached" ->
  let source,cache,calls,finish=make() in
  let a=Cache.get cache "x" in Cache.invalidate cache "x";
  let b=Cache.get cache "x" and c=Cache.get cache "y" in
  finish 1 (Error "retry pending");assert(Cache.in_flight cache=3);
  Cache.close cache;Cache.close cache;
  List.iter [a;b;c] ~f:(fun r -> assert(peek r=Some Request.Closed));
  assert(Cache.in_flight cache=0);assert(Cache.cached cache=0);no_alarms source cache;
  finish 0 (Ok 1);finish 2 (Error "late");advance source 100;
  assert(List.length !calls=3);Cache.invalidate cache "x";
  assert(peek(Cache.get cache "x")=Some Request.Closed);no_alarms source cache
| "reentrant_worker" ->
  let source=Time_source.create ~now:Time_ns.epoch () in
  let holder=ref None and joined=ref None and replacement=ref None in
  let old_gate=Ivar.create() and new_gate=Ivar.create() in
  let invocations=ref 0 in
  let worker _ ~attempt:_ =
    incr invocations;
    if !invocations=1 then begin
      let cache=Option.value_exn !holder in
      joined:=Some(Cache.get cache "x");Cache.invalidate cache "x";
      replacement:=Some(Cache.get cache "x");Ivar.read old_gate
    end else Ivar.read new_gate
  in
  let cache=Cache.create ~source:(Time_source.read_only source) ~policy:(policy()) ~worker in
  holder:=Some cache;let first=Cache.get cache "x" in
  assert(!invocations=2);assert(Cache.in_flight cache=2);
  Ivar.fill_exn new_gate (Ok 20);pump();Ivar.fill_exn old_gate (Ok 10);pump();
  assert(peek first=Some(Request.Value 10));assert(peek(Option.value_exn !joined)=peek first);
  assert(peek(Option.value_exn !replacement)=Some(Request.Value 20));
  assert(peek(Cache.get cache "x")=Some(Request.Value 20));no_alarms source cache;
  let holder=ref None in
  let worker _ ~attempt:_ = Cache.close(Option.value_exn !holder);return(Ok 1) in
  let cache=Cache.create ~source:(Time_source.read_only source) ~policy:(policy()) ~worker in
  holder:=Some cache;let r=Cache.get cache "close" in pump();
  assert(peek r=Some Request.Closed);no_alarms source cache
| "monitor_errors" ->
  let source=Time_source.create ~now:Time_ns.epoch () in
  let calls=ref 0 and old_monitor=ref None in
  let worker _ ~attempt =
    incr calls;
    if attempt=1 then failwith "sync"
    else if attempt=2 then begin
      let monitor=Monitor.current() in
      old_monitor:=Some monitor;
      upon (Time_source.after (Time_source.read_only source) (seconds 1))
        (fun () -> Monitor.send_exn monitor (Failure "async"));
      Deferred.never()
    end else return(Ok 8)
  in
  let cache=Cache.create ~source:(Time_source.read_only source) ~policy:(policy ~timeout:20()) ~worker in
  let r=Cache.get cache "x" in pump();assert(!calls=1);
  advance source 2;assert(!calls=2);advance source 3;assert(peek r=None);
  advance source 5;assert(peek r=Some(Request.Value 8));assert(!calls=3);
  Monitor.send_exn (Option.value_exn !old_monitor) (Failure "late monitor");pump();
  assert(peek(Cache.get cache "x")=Some(Request.Value 8));no_alarms source cache;
  let cache=Cache.create ~source:(Time_source.read_only source)
    ~policy:(policy ~attempts:1 ()) ~worker:(fun _ ~attempt:_ -> failwith "fail") in
  let r=Cache.get cache "x" in pump();assert(peek r=Some(Request.Failed "worker exception"))
| "alarm_ownership" ->
  let source=Time_source.create ~now:Time_ns.epoch () in
  let alarms=Alarm.create(Time_source.read_only source) in
  let trace=ref [] in
  let cancelled=Alarm.at alarms (at 2) (fun () -> trace:= "bad"::!trace) in
  let active=Alarm.at alarms (at 1) (fun () ->
    assert(Alarm.pending alarms=0);trace:= "first"::!trace;
    ignore(Alarm.at alarms (at 3) (fun () -> trace:= "third"::!trace))) in
  assert(Alarm.pending alarms=2);Alarm.cancel cancelled;Alarm.cancel cancelled;
  assert(Alarm.pending alarms=1);advance source 1;Alarm.cancel active;
  assert(!trace=["first"]);assert(Alarm.pending alarms=1);
  advance source 3;assert(!trace=["third";"first"]);assert(Alarm.pending alarms=0);
  assert(not(pending_source source));
  let source,cache,_,finish=make() in
  ignore(Cache.get cache "x");finish 0 (Error "retry");
  assert(Cache.pending_alarms cache=2);Cache.close cache;no_alarms source cache
| _ -> failwith "unknown check"
