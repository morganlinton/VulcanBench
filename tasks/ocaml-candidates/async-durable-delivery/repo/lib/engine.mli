type t
val create :
  clock:Clock.t ->
    policy:Policy.t -> worker:Worker.run -> journal:Journal.writer -> t
val submit : t -> key:string -> payload:string -> Ticket.t option
val close : t -> unit Async_kernel.Deferred.t
val abort : t -> unit
val in_flight : t -> int
val pending : t -> int
