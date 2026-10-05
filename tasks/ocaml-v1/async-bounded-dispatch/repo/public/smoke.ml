let () = let t=Pipeline.create ~concurrency:1 ~capacity:1 ~worker:(fun x->Async_kernel.Deferred.return(Ok x)) in
  assert(Pipeline.active t=0); assert(Pipeline.queued t=0)
