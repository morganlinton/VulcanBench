type step =
  | Field of int 
  | Index of int 
  | Left 
  | Right 
  | Some_value 
type reason =
  | Truncated 
  | Invalid_scalar 
  | Duplicate_field of int 
  | Missing_field of int 
  | Trailing 
  | Invalid_bounds 
type t = {
  at: int ;
  path: step list ;
  reason: reason }
exception Decode of t 
