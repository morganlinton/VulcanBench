type 'a presence = Required | Default of 'a
type _ t =
  | Atom : 'a Type_class.t -> 'a t
  | Pair : 'a t * 'b t -> ('a * 'b) t
  | Option : 'a t -> 'a option t
  | List : 'a t -> 'a list t
  | Record : ('a, 'r) fields * ('a -> 'r) -> 'r t
  | Map : 'a t * ('a -> 'b) * ('b -> 'a) -> 'b t
and (_, _) fields =
  | Nil : (unit, 'r) fields
  | Field : int * 'a t * 'a presence * ('r -> 'a) * ('b, 'r) fields -> (('a * 'b), 'r) fields

type step = Field_id of int | Index of int | Left | Right | Some_value | Mapped
type reason = Truncated | Invalid_tag | Duplicate_field of int | Missing_field of int
  | Trailing | Invalid_bounds | Invalid_frame_size | Frame_too_large
type error = { at : int; path : step list; reason : reason }
exception Decode of error
val validate : 'a t -> unit
val encode : 'a t -> 'a -> Common.buf
val writer : 'a t -> 'a Type_class.writer
val decode : 'a t -> Common.buf -> pos_ref:int ref -> limit:int -> ('a, error) result
val reader : 'a t -> 'a Type_class.reader
