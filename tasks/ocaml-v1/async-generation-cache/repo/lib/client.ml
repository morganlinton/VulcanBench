open Async_kernel
let fetch cache keys = Deferred.all(List.map (fun key -> Request.result(Cache.get cache key)) keys)
