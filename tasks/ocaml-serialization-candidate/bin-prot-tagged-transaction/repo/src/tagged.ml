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

let rec validate : type a. a t -> unit = function
 | Atom _ -> ()
 | Pair (a,b) -> validate a; validate b
 | Option a -> validate a
 | List a -> validate a
 | Map (a,_,_) -> validate a
 | Record (fs,_) -> validate_fields (Hashtbl.create 8) fs
and validate_fields : type a r. (int,unit) Hashtbl.t -> (a,r) fields -> unit = fun seen -> function
 | Nil -> ()
 | Field (id,s,_,_,rest) ->
   if id < 1 || id > 65535 || Hashtbl.mem seen id then invalid_arg "Tagged field id";
   Hashtbl.add seen id (); validate s; validate_fields seen rest
let unsigned b width n =
 if n < 0 || Int64.of_int n > (if width=2 then 65535L else 0xffffffffL) then invalid_arg "Tagged length";
 for i=0 to width-1 do Buffer.add_char b (Char.chr ((n lsr (8*i)) land 255)) done
let frame b s = unsigned b 4 (String.length s); Buffer.add_string b s
let string_of_buf buf = String.init (Bigarray.Array1.dim buf) (fun i -> buf.{i})
let rec payload : type a. a t -> a -> string = fun schema v ->
 let b=Buffer.create 32 in
 (match schema with
 | Atom tc -> Buffer.add_string b (string_of_buf (Utils.bin_dump tc.writer v))
 | Pair (a,c) -> frame b (payload a (fst v)); frame b (payload c (snd v))
 | Option s -> (match v with None -> Buffer.add_char b '\000' | Some x -> Buffer.add_char b '\001'; frame b (payload s x))
 | List s -> unsigned b 2 (List.length v); List.iter (fun x -> frame b (payload s x)) v
 | Map (s,_,project) -> Buffer.add_string b (payload s (project v))
 | Record (fs,_) ->
   let entries=List.sort (fun (a,_) (c,_) -> compare a c) (entries fs v) in
   unsigned b 2 (List.length entries); List.iter (fun (id,s) -> unsigned b 2 id; frame b s) entries);
 Buffer.contents b
and entries : type a r. (a,r) fields -> r -> (int*string) list = fun fs v -> match fs with
 | Nil -> []
 | Field (id,s,_,get,rest) -> (id,payload s (get v)) :: entries rest v
let encode schema v =
 validate schema; let s=payload schema v in let b=Common.create_buf (String.length s) in
 String.iteri (fun i c -> b.{i} <- c) s; b
let writer schema : _ Type_class.writer =
 { size=(fun v -> Bigarray.Array1.dim (encode schema v));
   write=(fun buf ~pos v -> let value=encode schema v in let n=Bigarray.Array1.dim value in
     if pos<0 || n > Bigarray.Array1.dim buf-pos then invalid_arg "Tagged writer bounds";
     Bigarray.Array1.blit value (Bigarray.Array1.sub buf pos n); pos+n) }
let decode schema buf ~pos_ref ~limit =
 validate schema;
 let start = !pos_ref in
 if start<0 || limit<start || limit>Bigarray.Array1.dim buf then
   Error {at=start; path=[]; reason=Invalid_bounds}
 else Error {at=start; path=[]; reason=Truncated}
let reader schema : _ Type_class.reader =
 { read=(fun buf ~pos_ref -> match decode schema buf ~pos_ref ~limit:(Bigarray.Array1.dim buf) with
   | Ok v -> v | Error e -> raise (Decode e));
   vtag_read=(fun _ ~pos_ref:_ _ -> invalid_arg "Tagged has no polymorphic variant reader") }
