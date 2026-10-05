open Core
open Core.Poly
open Async_kernel
let () =
  let source=Time_source.create ~now:Time_ns.epoch () in
  let policy=Policy.create ~ttl:Time_ns.Span.zero ~timeout:Time_ns.Span.second
    ~retry_delay:Time_ns.Span.second ~max_attempts:1 in
  let cache=Cache.create ~source:(Time_source.read_only source) ~policy
    ~worker:(fun _ ~attempt:_ -> return(Ok 3)) in
  assert(Cache.in_flight cache=0); assert(Cache.pending_alarms cache=0);
  Cache.close cache; Cache.close cache;
  assert(Deferred.peek(Request.result(Cache.get cache "x"))=Some Request.Closed)
