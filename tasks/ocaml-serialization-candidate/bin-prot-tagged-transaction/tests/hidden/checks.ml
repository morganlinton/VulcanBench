open Bin_prot
open Tagged
let check msg b = if not b then failwith msg
let len = Bigarray.Array1.dim
let buffer s = let b=Common.create_buf (String.length s) in String.iteri (fun i c -> b.{i}<-c) s; b
let bytes b = String.init (len b) (fun i->b.{i})
let concat bs = buffer (String.concat "" (List.map bytes bs))
let uint width n = String.init width (fun i->Char.chr ((n lsr (8*i)) land 255))
let framed s = uint 4 (String.length s)^s
let record es = buffer (uint 2 (List.length es)^String.concat "" (List.map (fun (id,s)->uint 2 id^framed s) es))
let int = Atom Type_class.bin_int
let bool = Atom Type_class.bin_bool
let text = Atom Type_class.bin_string
let get s b = match decode s b ~pos_ref:(ref 0) ~limit:(len b) with Ok x->x | Error _->failwith "decode failed"
let rt s v = let b=encode s v in check "round trip" (get s b = v)
let error_at s b start stop at path reason =
 let p=ref start in
 check "error value" (decode s b ~pos_ref:p ~limit:stop = Error {at;path;reason});
 check "error rollback" (!p=start)
let error s b at path reason = error_at s b 0 (len b) at path reason
let single = Record (Field (1,int,Required,(fun x->x),Nil),(fun (x,())->x))
type quote={id:int;name:string;urgent:bool}
let old=Record(Field(1,int,Required,(fun q->q.id),Field(2,text,Required,(fun q->q.name),Nil)),
 fun (id,(name,()))->{id;name;urgent=false})
let current=Record(Field(1,int,Required,(fun q->q.id),Field(2,text,Required,(fun q->q.name),Field(3,bool,Default false,(fun q->q.urgent),Nil))),
 fun (id,(name,(urgent,())))->{id;name;urgent})
let heterogeneous_migration () =
 let q={id=42;name="legacy";urgent=false} in check "migration" (get current (encode old q)=q);
 rt (Pair(List current,Option(Pair(int,bool)))) ([q;{q with urgent=true;id= -1000}],Some(99,true));
 let custom=Record(Field(17,Pair(text,List int),Required,(fun x->x),Field(2,Option bool,Default None,(fun _->None),Nil)),fun (x,(_,()))->x) in
 rt custom ("a\000z",[-999;0;65536]);
 let mapped=Map(Pair(int,text),(fun (i,s)->string_of_int i^":"^s),(fun s->int_of_string(String.sub s 0 1),String.sub s 2 (String.length s-2))) in
 rt (List mapped) ["3:hello";"4:"];
 rt (Record(Nil,fun ()->())) ();rt (List int) [];rt (Option text) None;
 check "declaration not wire order" (get single (record[99,"opaque";1,bytes(encode int 7)])=7)
let wire_compatibility () =
 rt int 42;
 check "legacy wire" (bytes(encode old {id=7;name="old";urgent=false})="\002\000\001\000\001\000\000\000\007\002\000\004\000\000\000\003old");
 check "option absent" (bytes(encode (Option bool) None)="\000");
 check "option present" (bytes(encode (Option bool)(Some true))="\001\001\000\000\000\001");
 check "sorted entries" (bytes(encode (Record(Field(9,int,Required,(fun x->x),Field(1,bool,Default true,(fun _->true),Nil)),fun(x,(_,()))->x)) 5)=
 "\002\000\001\000\001\000\000\000\001\009\000\001\000\000\000\005");
 let b=Utils.bin_dump ~header:true (writer current) {id=4;name="x";urgent=true} in
 check "dump equality" (bytes b=bytes(Tagged_stream.dump current {id=4;name="x";urgent=true}));
 let at=ref 0 in let read dst ~pos ~len:n = Bigarray.Array1.blit (Bigarray.Array1.sub b !at n) (Bigarray.Array1.sub dst pos n);at:= !at+n in
 let v=Utils.bin_read_stream ~read (reader current) in check "utils stream" (v={id=4;name="x";urgent=true});
 for i= -40 to 40 do rt (Pair(int,List text)) (i,[string_of_int i;"\000\255"]) done
let structured_bounds () =
 rt bool true;
 error (Option bool) (buffer "\002") 0 [] Invalid_tag;
 error (Option bool) (buffer "\001\004\000\000\000\001") 1 [Some_value] Truncated;
 error (List bool) (buffer "\001\000\255\255\255\255") 2 [Index 0] Truncated;
 error bool (buffer "\001\000") 1 [] Trailing;
 error single (record [9,"x";9,"y"]) 9 [Field_id 9] (Duplicate_field 9);
 error single (buffer "\002\000\009\000\001\000\000\000x\009\000") 9 [Field_id 9] (Duplicate_field 9);
 error single (record [2,"x"]) 9 [Field_id 1] (Missing_field 1);
 error (List(Option bool)) (buffer "\001\000\001\000\000\000\002") 6 [Index 0] Invalid_tag;
 error (Map(Option bool,(fun x->x),(fun x->x))) (buffer "\002") 0 [Mapped] Invalid_tag;
 let b=concat[buffer "prefix";encode single 4;buffer "sentinel"] in
 let p=ref 6 in check "absolute slice" (decode single b ~pos_ref:p ~limit:15=Ok 4);check "commit" (!p=15);
 error_at single b 6 14 10 [Field_id 1] Truncated;
 List.iter (fun(start,stop)->error_at int (buffer "x") start stop start [] Invalid_bounds) [(-1,1);(0,2);(1,0)];
 let no_atom=Record(Nil,fun()->()) in
 error no_atom (buffer "\000\000x") 2 [] Trailing;
 let b=encode current {id=7;name="old";urgent=true} in
 for stop=0 to len b-1 do let p=ref 0 in
   check "truncation sweep" (match decode current b ~pos_ref:p ~limit:stop with Error _->true|_->false);
   check "truncation rollback" (!p=0) done
let expect_same packet start p run =
 check "exception identity" (try ignore(run());false with exn->exn==packet);
 check "exception rollback" (!p=start)
let callback_transactions () =
 rt int 3;
 List.iter (fun packet ->
  let p=ref 2 in
  let s=Record(Field(1,int,Required,(fun x->x),Nil),fun(x,())->p:=987;raise packet) in
  let b=concat[buffer "xx";encode single 3] in
  expect_same packet 2 p (fun()->decode s b ~pos_ref:p ~limit:(len b));
  let s=Map(int,(fun _->p:=876;raise packet),(fun x->x)) in
  expect_same packet 2 p (fun()->decode s (buffer "xx\003") ~pos_ref:p ~limit:3))
 [Failure "user callback"; Decode {at=999;path=[Right];reason=Trailing}; Invalid_argument "user"];
 let p=ref 0 in
 let atom=Atom {Type_class.bin_int with reader={Type_class.bin_reader_int with read=(fun _ ~pos_ref:_ -> p:=555;raise (Failure "atom"))}} in
 let packet=Failure "atom" in
 let atom=match atom with Atom tc->Atom{tc with reader={tc.reader with read=(fun _ ~pos_ref:_->p:=555;raise packet)}}|_->assert false in
 expect_same packet 0 p (fun()->decode atom (buffer "\001") ~pos_ref:p ~limit:1);
 let p=ref 0 in let called=ref 0 in
 let s=Record(Nil,fun()->incr called;p:=99;()) in
 error s (buffer "\000\000x") 2 [] Trailing;check "no constructor on trailing" (!called=0);
 let s=Map(int,(fun x->incr called;x),(fun x->x)) in
 error s (buffer "\001x") 1 [Mapped] Trailing;check "no map on trailing" (!called=0);
 let s=Record(Field(1,int,Required,(fun x->x),Nil),fun(x,())->
  check "reentrant" (get bool (buffer "\001"));p:=999;x) in
 check "success" (decode s (encode single 8) ~pos_ref:p ~limit:9=Ok 8);check "success overrides callback" (!p=9);
 let count=ref 0 in
 let nested=Map(int,(fun x->incr count;x),(fun x->x)) in
 let s=Record(Field(1,nested,Required,(fun x->x),Nil),fun(x,())->x) in
 error s (concat[record[1,"\001"];buffer "x"]) 9 [] Trailing;
 check "record scans before child callbacks" (!count=0)
let stream_transactions () =
 let q={id=7;name="z";urgent=true} in
 let first=Tagged_stream.dump current q and second=Tagged_stream.dump int 42 in
 let b=concat[buffer "xx";first;second;buffer "z"] in let p=ref 2 in
 check "first frame" (Tagged_stream.decode_next current b ~pos_ref:p ~limit:(len b) ~max_size:100=Ok(Some q));
 check "first commit" (!p=2+len first);
 check "second frame" (Tagged_stream.decode_next int b ~pos_ref:p ~limit:(len b) ~max_size:100=Ok(Some 42));
 check "second commit" (!p=2+len first+len second);
 let calls=ref 0 in let s=Map(current,(fun x->incr calls;x),(fun x->x)) in
 for stop=2 to 2+len first-1 do let p=ref 2 in
  check "incomplete" (Tagged_stream.decode_next s b ~pos_ref:p ~limit:stop ~max_size:100=Ok None);
  check "incomplete cursor" (!p=2) done;check "no callbacks on incomplete" (!calls=0);
 let err data limit max_size reason = let p=ref 2 in let b=concat[buffer "xx";data] in
  check "frame error" (Tagged_stream.decode_next int b ~pos_ref:p ~limit ~max_size=Error{at=2;path=[];reason});check "frame rollback" (!p=2) in
 let hdr n=Utils.bin_dump Type_class.bin_writer_int64_bits n in
 err (hdr (-1L)) 10 100 Invalid_frame_size;
 err (hdr Int64.max_int) 10 max_int Invalid_frame_size;
 err (hdr 101L) 10 100 Frame_too_large;
 let p=ref 2 in let bad=concat[buffer "xx";hdr 1L;buffer "\002"] in
 check "absolute payload error" (Tagged_stream.decode_next (Option bool) bad ~pos_ref:p ~limit:11 ~max_size:10=Error{at=10;path=[];reason=Invalid_tag});check "payload rollback" (!p=2);
 let packet=Decode{at=0;path=[];reason=Trailing} in
 let p=ref 0 in let s=Map(int,(fun _->p:=555;raise packet),(fun x->x)) in
 expect_same packet 0 p (fun()->Tagged_stream.decode_next s (Tagged_stream.dump int 4) ~pos_ref:p ~limit:9 ~max_size:10);
 let p=ref 3 in let b=Tagged_stream.dump (Record(Nil,fun()->())) () in
 let buf=concat[buffer "xxx";b] in check "empty record frame" (Tagged_stream.decode_next (Record(Nil,fun()->())) buf ~pos_ref:p ~limit:(len buf) ~max_size:2=Ok(Some()));check "empty commit" (!p=len buf)
let bounded_atoms () =
 rt int 4;
 let inspected=ref false in
 let tc={Type_class.bin_bool with reader={Type_class.bin_reader_bool with read=(fun b ~pos_ref ->
   check "leaf view length" (len b=1);check "leaf origin" (b.{0}='\001');check "leaf local cursor" (!pos_ref=0);
   check "no sentinel access" (try ignore(b.{1});false with Invalid_argument _->true);
   inspected:=true;b.{0}<-'\000';pos_ref:=1;true)}} in
 let b=buffer "xx\001z" and p=ref 2 in
 check "leaf decode" (decode (Atom tc) b ~pos_ref:p ~limit:3=Ok true);
 check "leaf zero copy alias" (!inspected && b.{2}='\000');check "leaf commit" (!p=3);
 List.iter(fun n->let tc={Type_class.bin_int with reader={Type_class.bin_reader_int with read=(fun _ ~pos_ref->pos_ref:=n;1)}} in
  if n=0 then error (Atom tc) (buffer "\001") 0 [] Trailing
  else error (Atom tc) (buffer "\001") 0 [] Invalid_bounds) [0;-1;2];
 let p=ref 2 in let packet=Failure "leaf custom" in
 let tc={Type_class.bin_int with reader={Type_class.bin_reader_int with read=(fun _ ~pos_ref->pos_ref:=100;p:=999;raise packet)}} in
 expect_same packet 2 p (fun()->decode (Atom tc) (buffer "xx\001z") ~pos_ref:p ~limit:3);
 let b=record[1,"\001"] in let p=ref 0 in
 let packet=Decode{at=555;path=[Left];reason=Truncated} in
 let tc={Type_class.bin_int with reader={Type_class.bin_reader_int with read=(fun _ ~pos_ref:_->p:=444;raise packet)}} in
 let s=Record(Field(1,Atom tc,Default 99,(fun x->x),Nil),fun(x,())->x) in
 expect_same packet 0 p (fun()->(reader s).read b ~pos_ref:p);
 let p=ref 0 in check "reader returned fault" (try ignore((reader single).read (buffer "\000\000") ~pos_ref:p);false with Decode{reason=Missing_field 1;_}->true|_->false);check "reader fault cursor" (!p=0)
let unknown_allocation () =
 rt int 9;
 let b=record[65000,String.make(2*1024*1024)'x';1,"\008"] in
 Gc.full_major();let before=Gc.allocated_bytes() in let got=get single b in
 let allocated=Gc.allocated_bytes()-.before in
 check "unknown value" (got=8);check "unknown heap allocation" (allocated<65536.);
 Printf.printf "unknown_heap_bytes=%.0f\n" allocated;
 error single (record[65000,"opaque";65000,"again";1,"\008"]) 14 [Field_id 65000] (Duplicate_field 65000)
let schema_precedence () =
 rt int 4;
 let invalid id=Record(Field(id,int,Required,(fun x->x),Nil),fun(x,())->x) in
 List.iter(fun id->let p=ref (-1) in
  check "schema before bounds" (try ignore(decode (List(invalid id)) (buffer "") ~pos_ref:p ~limit:99);false with Invalid_argument _->true);
  check "schema cursor" (!p= -1);
  check "schema before max size" (try ignore(Tagged_stream.decode_next (invalid id) (buffer "") ~pos_ref:p ~limit:99 ~max_size:(-1));false with Invalid_argument s->s="Tagged field id"|_->false)) [0;65536];
 let dup=Record(Field(1,int,Required,(fun x->x),Field(1,int,Required,(fun x->x),Nil)),fun(x,(_,()))->x) in
 check "duplicate schema" (try ignore(decode dup (buffer "") ~pos_ref:(ref 0) ~limit:0);false with Invalid_argument _->true);
 let p=ref 8 in check "vtag invalid" (try ignore((reader int).vtag_read (buffer "") ~pos_ref:p 1);false with Invalid_argument _->true);check "vtag cursor" (!p=8);
 let p=ref 9 in check "max size before bounds" (try ignore(Tagged_stream.decode_next int (buffer "") ~pos_ref:p ~limit:0 ~max_size:(-1));false with Invalid_argument _->true);check "max cursor" (!p=9)
let ordinary () =
 let b=Utils.bin_dump ~header:true Type_class.bin_writer_int 42 in check "header" (len b=9);
 let p=ref 0 in check "size header" (Utils.bin_read_size_header b ~pos_ref:p=1);
 check "standard reader" (Read.bin_read_int b ~pos_ref:p=42);check "standard cursor" (!p=9);
 check "new writer stable" (bytes(encode (Pair(int,bool))(42,true))="\001\000\000\000\042\001\000\000\000\001");
 check "default encoder emits" (len(encode single 8)=9)
let () = match Sys.argv.(1) with
 | "heterogeneous_migration"->heterogeneous_migration()
 | "wire_compatibility"->wire_compatibility()
 | "structured_bounds"->structured_bounds()
 | "callback_transactions"->callback_transactions()
 | "stream_transactions"->stream_transactions()
 | "bounded_atoms"->bounded_atoms()
 | "unknown_allocation"->unknown_allocation()
 | "schema_precedence"->schema_precedence()
 | "ordinary"->ordinary()
 | _->failwith "unknown check"
