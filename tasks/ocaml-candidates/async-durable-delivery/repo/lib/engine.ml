type t = Runtime.t
let rec pump r =
  if r.Runtime.pumping
  then r.again <- true
  else
    (r.pumping <- true;
     r.again <- true;
     while r.again do
       (r.again <- false;
        Dispatcher.step r;
        Committer.step r;
        Runtime.check_closed r)
       done;
     r.pumping <- false)
let create ~clock ~policy ~worker ~journal =
  let r = Runtime.create ~clock ~policy ~worker ~journal in
  r.wakeup <- ((fun () -> pump r)); r
let submit r ~key ~payload =
  if (not r.Runtime.accepting) || (r.pending >= (r.policy).capacity)
  then None
  else
    (let seq = r.next_seq in
     r.next_seq <- (seq + 1);
     r.pending <- (r.pending + 1);
     (let ticket = Ticket.create seq in
      let j =
        {
          Job.seq = seq;
          key;
          payload;
          ticket;
          phase = Queued;
          generation = 0;
          attempt = 0;
          deadline = 0;
          alarm = None
        } in
      Ticket.on_cancel ticket (fun () -> Runtime.cancel r j);
      r.jobs <- (r.jobs @ [j]);
      pump r;
      Some ticket))
let close r =
  r.Runtime.accepting <- false; pump r; Async_kernel.Ivar.read r.closed
let abort r = Runtime.stop r Outcome.Aborted; pump r
let in_flight r = r.Runtime.active
let pending r = r.Runtime.pending
