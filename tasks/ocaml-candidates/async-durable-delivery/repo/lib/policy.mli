type t =
  {
  slots: int ;
  capacity: int ;
  timeout: int ;
  retry_delay: int ;
  attempts: int }
val create :
  slots:int ->
    capacity:int -> timeout:int -> retry_delay:int -> attempts:int -> t
