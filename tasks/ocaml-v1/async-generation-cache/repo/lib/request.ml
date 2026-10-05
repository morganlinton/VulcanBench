open Async_kernel
type 'a outcome = Value of 'a | Failed of string | Timed_out | Cancelled | Closed
type 'a t = { result:'a outcome Ivar.t; mutable hook:unit -> unit }
let create () = {result=Ivar.create ();hook=(fun () -> ())}
let result t = Ivar.read t.result
let is_pending t = not(Ivar.is_full t.result)
let resolve t value = if is_pending t then Ivar.fill_exn t.result value
let on_cancel t hook = t.hook<-hook
let cancel t = if is_pending t then (resolve t Cancelled; let hook=t.hook in
  t.hook<-(fun () -> ());hook())
