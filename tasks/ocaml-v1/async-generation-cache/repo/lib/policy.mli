type t = private {
  ttl : Core.Time_ns.Span.t;
  timeout : Core.Time_ns.Span.t;
  retry_delay : Core.Time_ns.Span.t;
  max_attempts : int;
}
val create : ttl:Core.Time_ns.Span.t -> timeout:Core.Time_ns.Span.t ->
  retry_delay:Core.Time_ns.Span.t -> max_attempts:int -> t
