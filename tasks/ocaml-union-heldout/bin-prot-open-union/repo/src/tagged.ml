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
(* Private parser exceptions cannot capture a user-raised Decode. *)
exception Fault of error
type cursor = { buf:Common.buf; mutable pos:int; stop:int; path:step list }
let fault c at reason = raise (Fault {at;path=c.path;reason})
let take c n =
 let at=c.pos in if n<0 || n>c.stop-c.pos then fault c at Truncated;
 c.pos <- c.pos+n; at
let unsigned_read c width =
 let at=take c width in let n=ref 0 in
 for i=0 to width-1 do n := !n lor (Char.code c.buf.{at+i} lsl (8*i)) done; !n
let descend c step = {c with path=c.path @ [step]}
let framed c step =
 let sub=descend c step in let at=c.pos in let n=unsigned_read sub 4 in
 if n>c.stop-sub.pos then fault sub at Truncated;
 c.pos <- sub.pos+n; {sub with stop=c.pos}
let exact c = if c.pos<>c.stop then fault c c.pos Trailing
let rec value : type a. a t -> cursor -> a = fun schema c ->
 let v : a = match schema with
 | Atom tc ->
   let at=c.pos in let n=c.stop-at in let local=ref 0 in
   let v=tc.reader.read (Bigarray.Array1.sub c.buf at n) ~pos_ref:local in
   if !local<0 || !local>n then fault c at Invalid_bounds;
   c.pos <- at + !local; v
 | Pair (a,b) ->
   let left=value a (framed c Left) in let right=value b (framed c Right) in left,right
 | Option s ->
   let at=take c 1 in (match c.buf.{at} with
    | '\000' -> None | '\001' -> Some (value s (framed c Some_value))
    | _ -> fault c at Invalid_tag)
 | List s ->
   let n=unsigned_read c 2 in let rec loop i acc =
    if i=n then List.rev acc else let v=value s (framed c (Index i)) in loop (i+1) (v::acc) in
   loop 0 []
 | Record (fs,make) ->
   let count=unsigned_read c 2 in let index=Hashtbl.create 8 in
   for _i=1 to count do
    let at=c.pos in let id=unsigned_read c 2 in
    if Hashtbl.mem index id then fault (descend c (Field_id id)) at (Duplicate_field id);
    let sub=framed c (Field_id id) in Hashtbl.add index id sub
   done;
   exact c;
   let tuple=fields fs index c in make tuple
 | Map (s,make,_) ->
   let sub=descend c Mapped in let v=value s sub in c.pos <- sub.pos;
   exact c; make v
 in exact c; v
and fields : type a r. (a,r) fields -> (int,cursor) Hashtbl.t -> cursor -> a = fun fs index parent ->
 match fs with
 | Nil -> ()
 | Field (id,s,presence,_,rest) ->
   let head=match Hashtbl.find_opt index id with
    | Some c -> value s c
    | None -> (match presence with Default v -> v | Required ->
        fault (descend parent (Field_id id)) parent.stop (Missing_field id)) in
   let tail=fields rest index parent in head,tail
let decode schema buf ~pos_ref ~limit =
 let start = !pos_ref in
 try
  validate schema;
  if start<0 || limit<start || limit>Bigarray.Array1.dim buf then
   Error {at=start;path=[];reason=Invalid_bounds}
  else
   (try
     let v=value schema {buf;pos=start;stop=limit;path=[]} in
     pos_ref := limit; Ok v
    with Fault e -> pos_ref := start; Error e)
 with exn -> pos_ref := start; raise exn
let reader schema : _ Type_class.reader =
 { read=(fun buf ~pos_ref -> match decode schema buf ~pos_ref ~limit:(Bigarray.Array1.dim buf) with
   | Ok v -> v | Error e -> raise (Decode e));
   vtag_read=(fun _ ~pos_ref:_ _ -> invalid_arg "Tagged has no polymorphic variant reader") }
