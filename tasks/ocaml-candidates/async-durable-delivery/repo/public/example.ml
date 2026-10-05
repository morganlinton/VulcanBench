let () =
  let clock = Clock.create () in
  let policy =
    Policy.create ~slots:2 ~capacity:4 ~timeout:5 ~retry_delay:2 ~attempts:2 in
  let worker ~key:_ ~payload ~attempt:_ =
    Async_kernel.Deferred.return (Ok payload) in
  let journal _ = Async_kernel.Deferred.return (Ok ()) in
  let t = Engine.create ~clock ~policy ~worker ~journal in
  let a = Option.get (Service.enqueue t "quotes" "v1") in
  ignore (Service.drain t);
  Async_kernel.Async_kernel_scheduler.Expert.run_cycles_until_no_jobs_remain
    ();
  assert
    ((Async_kernel.Deferred.peek (Ticket.result a)) =
       (Some (Outcome.Committed (Outcome.Succeeded "v1"))))
