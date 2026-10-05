type entry = {
  seq: int ;
  key: string ;
  outcome: Outcome.outcome }
type writer = entry -> (unit, string) result Async_kernel.Deferred.t
