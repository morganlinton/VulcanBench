type t =
  {
  slots: int ;
  capacity: int ;
  timeout: int ;
  retry_delay: int ;
  attempts: int }
let create ~slots ~capacity ~timeout ~retry_delay ~attempts =
  if
    (slots < 1) ||
      ((capacity < 1) ||
         ((timeout < 1) || ((retry_delay < 0) || (attempts < 1))))
  then invalid_arg "policy";
  { slots; capacity; timeout; retry_delay; attempts }
