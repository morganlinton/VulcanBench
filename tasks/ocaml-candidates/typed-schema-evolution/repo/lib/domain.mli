type quote = {
  id: int64 ;
  label: string ;
  urgent: bool }
val v1 : quote Schema.t
val v2 : quote Schema.t
val batch : quote list Schema.t
