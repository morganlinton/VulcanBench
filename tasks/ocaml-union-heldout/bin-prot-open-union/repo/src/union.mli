type 'r case = Case : { tag : int; codec : 'a Tagged.t; project : 'r -> 'a option; inject : 'a -> 'r } -> 'r case
type 'r unknown = { project_unknown : 'r -> (int * Common.buf) option; inject_unknown : int -> Common.buf -> 'r }
type 'r t = Closed of 'r case list | Open of 'r case list * 'r unknown
type reason = Invalid_bounds | Truncated | Invalid_tag | Unknown_tag of int | Trailing | Payload of Tagged.error
type error = { at : int; reason : reason }
exception Decode of error
val validate : 'r t -> unit
val encode : 'r t -> 'r -> Common.buf
val decode : 'r t -> Common.buf -> pos_ref:int ref -> limit:int -> ('r, error) result
val writer : 'r t -> 'r Type_class.writer
val reader : 'r t -> 'r Type_class.reader
val type_class : shape:Shape.t -> 'r t -> 'r Type_class.t
