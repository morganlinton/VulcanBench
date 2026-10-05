type t =
  {
  clock: Clock.t ;
  policy: Policy.t ;
  worker: Worker.run ;
  journal: Journal.writer ;
  mutable jobs: Job.t list ;
  mutable next_seq: int ;
  mutable commit_seq: int ;
  mutable pending: int ;
  mutable active: int ;
  mutable busy: bool ;
  mutable accepting: bool ;
  mutable stopped: Outcome.completion option ;
  closed: unit Async_kernel.Ivar.t ;
  mutable pumping: bool ;
  mutable again: bool ;
  mutable wakeup: unit -> unit }
let create ~clock ~policy ~worker ~journal =
  {
    clock;
    policy;
    worker;
    journal;
    jobs = [];
    next_seq = 0;
    commit_seq = 0;
    pending = 0;
    active = 0;
    busy = false;
    accepting = true;
    stopped = None;
    closed = (Async_kernel.Ivar.create ());
    pumping = false;
    again = false;
    wakeup = (fun () -> ())
  }
let retire_alarm (j : Job.t) =
  Option.iter Clock.cancel j.alarm; j.alarm <- None
let check_closed r =
  if
    (not r.accepting) &&
      ((r.pending = 0) &&
         ((not r.busy) && (not (Async_kernel.Ivar.is_full r.closed))))
  then Async_kernel.Ivar.fill_exn r.closed ()
let settle r (j : Job.t) value =
  if j.phase <> Job.Done
  then
    (retire_alarm j;
     j.generation <- (j.generation + 1);
     j.phase <- Job.Done;
     r.pending <- (r.pending - 1);
     r.jobs <- (List.filter (fun other -> other != j) r.jobs);
     Ticket.settle j.ticket value)
let stop r value =
  if r.stopped = None
  then
    (r.stopped <- (Some value);
     r.accepting <- false;
     List.iter (fun j -> settle r j value) r.jobs;
     r.active <- 0;
     check_closed r)
let cancel r (j : Job.t) =
  match j.phase with
  | Job.Done | Committing -> ()
  | _ ->
      ((match j.phase with
        | Running _ -> r.active <- (r.active - 1)
        | _ -> ());
       retire_alarm j;
       j.generation <- (j.generation + 1);
       j.phase <- (Ready Outcome.Cancelled);
       r.wakeup ())
