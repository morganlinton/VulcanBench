open Core
type t = {
  ttl : Time_ns.Span.t;
  timeout : Time_ns.Span.t;
  retry_delay : Time_ns.Span.t;
  max_attempts : int;
}
let create ~ttl ~timeout ~retry_delay ~max_attempts =
  if Time_ns.Span.(ttl < zero || timeout <= zero || retry_delay <= zero)
     || max_attempts < 1 then invalid_arg "invalid cache policy";
  {ttl;timeout;retry_delay;max_attempts}
