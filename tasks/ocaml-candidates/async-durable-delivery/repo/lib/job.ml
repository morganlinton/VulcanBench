type phase =
  | Queued 
  | Running of int 
  | Backoff 
  | Ready of Outcome.outcome 
  | Committing 
  | Done 
type t =
  {
  seq: int ;
  key: string ;
  payload: string ;
  ticket: Ticket.t ;
  mutable phase: phase ;
  mutable generation: int ;
  mutable attempt: int ;
  mutable deadline: int ;
  mutable alarm: Clock.alarm option }
