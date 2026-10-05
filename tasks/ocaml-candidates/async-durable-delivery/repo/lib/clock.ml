type alarm = {
  at: int ;
  serial: int ;
  f: unit -> unit ;
  mutable live: bool }
type t = {
  mutable now: int ;
  mutable serial: int ;
  mutable alarms: alarm list }
let create () = { now = 0; serial = 0; alarms = [] }
let now t = t.now
let cancel a = a.live <- false
let pending t =
  List.fold_left (fun n a -> if a.live then n + 1 else n) 0 t.alarms
let rec drain t =
  match List.sort
          (fun a b ->
             let c = compare a.at b.at in
             if c = 0 then compare a.serial b.serial else c)
          (List.filter (fun a -> a.live) t.alarms)
  with
  | a::_ when a.at <= t.now -> (a.live <- false; a.f (); drain t)
  | _ -> t.alarms <- (List.filter (fun a -> a.live) t.alarms)
let at t at f =
  let a = { at; serial = (t.serial); f; live = true } in
  t.serial <- (t.serial + 1);
  t.alarms <- (a :: (t.alarms));
  if at <= t.now then drain t;
  a
let advance t n =
  if n < t.now then invalid_arg "clock reversal"; t.now <- n; drain t
