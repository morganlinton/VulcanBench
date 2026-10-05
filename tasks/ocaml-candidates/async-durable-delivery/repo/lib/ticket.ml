type t =
  {
  seq: int ;
  result: Outcome.completion Async_kernel.Ivar.t ;
  mutable cancel: unit -> unit }
let create seq =
  { seq; result = (Async_kernel.Ivar.create ()); cancel = (fun () -> ()) }
let seq t = t.seq
let result t = Async_kernel.Ivar.read t.result
let cancel t = t.cancel ()
let on_cancel t f = t.cancel <- f
let settle t v =
  if not (Async_kernel.Ivar.is_full t.result)
  then (t.cancel <- ((fun () -> ())); Async_kernel.Ivar.fill_exn t.result v)
