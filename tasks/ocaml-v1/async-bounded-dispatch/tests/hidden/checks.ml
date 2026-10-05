open Async_kernel
let pump () = Async_kernel_scheduler.Expert.run_cycles_until_no_jobs_remain ()
let peek d = Deferred.peek d
let make concurrency capacity =
  let started=ref [] in let gates=Hashtbl.create 8 in
  let worker x = started:= !started @ [x]; let gate=Ivar.create() in Hashtbl.add gates x gate; Ivar.read gate in
  let t=Pipeline.create ~concurrency ~capacity ~worker in t,started,(fun x result->Ivar.fill_exn (Hashtbl.find gates x) result; pump())
let () = match Sys.argv.(1) with
| "empty_limits" -> let t,_,_=make 1 0 in assert(Pipeline.active t=0); assert(Pipeline.queued t=0); assert(peek(Pipeline.close t)=Some()); (try ignore(make 0 1); assert false with Invalid_argument _->())
| "bounded_fifo" -> let t,started,finish=make 1 2 in let a=Pipeline.submit t 1 and b=Pipeline.submit t 2 and c=Pipeline.submit t 3 in assert(peek(Pipeline.submit t 4)=Some(Error "full")); assert(!started=[1]); assert(Pipeline.queued t=2); finish 1 (Ok 11); assert(!started=[1;2]); assert(peek a=Some(Ok 11)); assert(peek b=None); finish 2 (Ok 22); finish 3 (Ok 33); assert(peek c=Some(Ok 33))
| "close_drain" -> let t,started,finish=make 2 3 in let a=Pipeline.submit t 1 and b=Pipeline.submit t 2 and c=Pipeline.submit t 3 in let done_=Pipeline.close t in assert(peek done_=None); assert(peek(Pipeline.submit t 4)=Some(Error "closed")); finish 2 (Ok 20); assert(!started=[1;2;3]); assert(peek b=Some(Ok 20)); assert(peek a=None); finish 3 (Ok 30); assert(peek done_=None); finish 1 (Ok 10); assert(peek done_=Some()); assert(peek c=Some(Ok 30)); assert(Pipeline.active t=0)
| "abort_pending" -> let t,started,finish=make 1 2 in let a=Pipeline.submit t 1 and b=Pipeline.submit t 2 in let done_=Pipeline.abort t in assert(peek b=Some(Error "aborted")); assert(peek done_=None); finish 1 (Ok 8); assert(peek a=Some(Ok 8)); assert(peek done_=Some()); assert(!started=[1]); assert(Pipeline.queued t=0)
| "errors_release" -> let t,started,finish=make 1 1 in let a=Pipeline.submit t 1 and b=Pipeline.submit t 2 in finish 1 (Error "failed"); assert(peek a=Some(Error "failed")); assert(!started=[1;2]); finish 2 (Ok 2); assert(peek b=Some(Ok 2)); let t=Pipeline.create ~concurrency:1 ~capacity:1 ~worker:(fun _->failwith "worker") in let d=Pipeline.submit t 4 in pump(); assert(peek d=Some(Error "worker exception")); assert(Pipeline.active t=0)
| "close_then_abort" -> let t,started,finish=make 1 2 in ignore(Pipeline.submit t 1); let b=Pipeline.submit t 2 in let d=Pipeline.close t in let e=Pipeline.abort t in assert(peek b=Some(Error "aborted")); assert(peek d=None); finish 1 (Ok 1); assert(peek d=Some()); assert(peek e=Some()); assert(!started=[1]); assert(peek(Pipeline.close t)=Some())
| _->failwith "unknown case"
