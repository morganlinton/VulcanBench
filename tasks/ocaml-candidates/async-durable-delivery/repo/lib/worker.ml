type run =
  key:string ->
    payload:string ->
      attempt:int -> (string, string) result Async_kernel.Deferred.t
