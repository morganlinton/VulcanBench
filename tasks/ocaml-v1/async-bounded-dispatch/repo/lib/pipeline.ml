open Async_kernel
type t = { worker : int -> (int,string) result Deferred.t; mutable count:int }
let create ~concurrency ~capacity ~worker =
  if concurrency<=0 || capacity<0 then invalid_arg "limits";
  {worker;count=0}
let submit t value = t.count<-t.count+1; let d=t.worker value in Deferred.upon d (fun _->t.count<-t.count-1); d
let close _ = Deferred.return ()
let abort _ = Deferred.return ()
let active t = t.count
let queued _ = 0
