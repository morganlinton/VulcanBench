let () =
  let clock = Clock.create () in
  let policy =
    Policy.create ~slots:2 ~capacity:4 ~timeout:5 ~retry_delay:2 ~attempts:2 in
  let worker ~key:_ ~payload ~attempt:_ =
    Async_kernel.Deferred.return (Ok payload) in
  let journal _ = Async_kernel.Deferred.return (Ok ()) in
  let engine = Engine.create ~clock ~policy ~worker ~journal in
  assert ((Async_kernel.Deferred.peek (Engine.close engine)) = (Some ()))
