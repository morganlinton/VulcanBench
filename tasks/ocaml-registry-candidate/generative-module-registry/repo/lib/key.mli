type (_, _) equal = Refl : ('a, 'a) equal
type 'a t
module type Payload = sig type t val name : string end
module type S = sig type value val key : value t end
module Make (P : Payload) () : S with type value = P.t
val name : 'a t -> string
val same : 'a t -> 'b t -> ('a, 'b) equal option
