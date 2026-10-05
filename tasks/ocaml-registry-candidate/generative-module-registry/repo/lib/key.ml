type (_, _) equal = Refl : ('a, 'a) equal
type _ witness = ..
type 'a t = { token : 'a witness; label : string; prove : 'b. 'b witness -> ('a, 'b) equal option }
module type Payload = sig type t val name : string end
module type S = sig type value val key : value t end
module Make (P : Payload) () = struct
 type value = P.t
 type _ witness += Token : value witness
 let key = {token=Token;label=P.name;prove=(fun _ -> None)}
end
let name k = k.label
let same _ _ = None
