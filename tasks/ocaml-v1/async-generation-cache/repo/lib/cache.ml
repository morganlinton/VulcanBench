type 'a t = { mutable closed:bool }
let create ~source:_ ~policy:_ ~worker:_ = {closed=false}
let get t _ = let request=Request.create () in
  Request.resolve request (if t.closed then Request.Closed else Request.Failed "not implemented");
  request
let invalidate _ _ = ()
let close t = t.closed<-true
let in_flight _ = 0
let cached _ = 0
let pending_alarms _ = 0
