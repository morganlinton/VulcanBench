type 'a presence =
  | Required 
  | Default of 'a 
type _ t =
  | Bool: bool t 
  | Int64: int64 t 
  | Text: string t 
  | Pair: 'a t * 'b t -> ('a * 'b) t 
  | Option: 'a t -> 'a option t 
  | List: 'a t -> 'a list t 
  | Record: ('a, 'r) fields * ('a -> 'r) -> 'r t 
and (_, _) fields =
  | Nil: (unit, 'r) fields 
  | Field: int * 'a t * 'a presence * ('r -> 'a) * ('b, 'r) fields ->
  (('a * 'b), 'r) fields 
val validate : 'a t -> unit
