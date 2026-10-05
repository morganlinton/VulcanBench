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
val create :
  clock:Clock.t ->
    policy:Policy.t -> worker:Worker.run -> journal:Journal.writer -> t
val retire_alarm : Job.t -> unit
val check_closed : t -> unit
val settle : t -> Job.t -> Outcome.completion -> unit
val stop : t -> Outcome.completion -> unit
val cancel : t -> Job.t -> unit
