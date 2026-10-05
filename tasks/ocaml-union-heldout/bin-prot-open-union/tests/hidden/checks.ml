open Bin_prot
module U = Union
module T = Tagged
let check b = if not b then failwith "check"
let dim = Bigarray.Array1.dim
let bytes s = let b=Common.create_buf (String.length s) in String.iteri (fun i c -> b.{i}<-c) s; b
let str b = String.init (dim b) (fun i -> b.{i})
let fill n = bytes (String.make n '\165')
let put b at n v = for i=0 to n-1 do b.{at+i} <- Char.chr ((v lsr (8*i)) land 255) done
let frame tag payload = let b=Common.create_buf (6+dim payload) in put b 0 2 tag;put b 2 4 (dim payload);Bigarray.Array1.blit payload (Bigarray.Array1.sub b 6 (dim payload)); b
let concat xs = bytes (String.concat "" (List.map str xs))
let err expected at = function Error e -> check (e.U.at=at && e.reason=expected) | Ok _ -> failwith "expected error"
let ok = function Ok v -> v | Error _ -> failwith "unexpected error"
let invalid f = match f () with _ -> failwith "expected invalid" | exception Invalid_argument _ -> ()
let identical exn f = match f () with _ -> failwith "expected exception" | exception e -> check (e==exn)
let int = T.Atom Type_class.bin_int
let string = T.Atom Type_class.bin_string
let unit = T.Atom Type_class.bin_unit
type message = Number of int | Text of string | Pair of int * string | Unknown of int * Common.buf
let number = U.Case {tag=12;codec=int;project=(function Number n -> Some n | _ -> None);inject=(fun n -> Number n)}
let text = U.Case {tag=2;codec=string;project=(function Text s -> Some s | _ -> None);inject=(fun s -> Text s)}
let pair = U.Case {tag=65000;codec=T.Pair(int,string);project=(function Pair(a,b)->Some(a,b)|_->None);inject=(fun (a,b)->Pair(a,b))}
let closed = U.Closed [number;text;pair]
let fallback = {U.project_unknown=(function Unknown(t,b)->Some(t,b)|_->None);inject_unknown=(fun t b->Unknown(t,b))}
let opened = U.Open ([number;text;pair],fallback)
let round s v = let b=U.encode s v in let p=ref 0 in let out=ok(U.decode s b ~pos_ref:p ~limit:(dim b)) in check(!p=dim b); out
let typed_dispatch () =
 check (round closed (Number 42)=Number 42); check(round closed (Text "abc")=Text "abc");check(round closed (Pair(1337,"x"))=Pair(1337,"x"));
 let b=U.encode closed (Text "a") in check(str b=str(frame 2 (T.encode string "a")));
 let shuffled=U.Closed [pair;number;text] in check(str(U.encode shuffled (Text "a"))=str b);
 let tc=U.type_class ~shape:Shape.bin_shape_int closed in check(tc.shape==Shape.bin_shape_int);
 let nested=T.List (T.Option (T.Atom tc)) in
 let v=[Some(Number 99);None;Some(Pair(5,""))] in let b=T.encode nested v in let p=ref 0 in check(ok(T.decode nested b ~pos_ref:p ~limit:(dim b))=v)
let projection_order () =
 let seen=ref [] and payload=ref 0 and unknown=ref 0 in
 let atom={Type_class.bin_int with writer={Type_class.bin_int.writer with size=(fun n->incr payload;Type_class.bin_int.writer.size n)}} in
 let c tag yes=U.Case{tag;codec=T.Atom atom;project=(fun _->seen:= !seen@[tag];if yes then Some 1 else None);inject=(fun _->())} in
 let f={U.project_unknown=(fun _->incr unknown;Some(90,bytes ""));inject_unknown=(fun _ _->())} in
 ignore(U.encode (U.Open([c 9 true;c 3 false;c 5 false],f)) ());check(!seen=[9;3;5] && !unknown=0 && !payload=1);
 seen:=[];payload:=0;invalid(fun()->U.encode(U.Open([c 9 true;c 3 true;c 5 false],f))());check(!seen=[9;3;5] && !payload=0 && !unknown=0);
 seen:=[];ignore(U.encode(U.Open([c 9 false;c 3 false],f))());check(!seen=[9;3] && !unknown=1);
 let exn=Failure "project" in identical exn (fun()->U.encode(U.Closed[U.Case{tag=1;codec=int;project=(fun _->raise exn);inject=(fun _->())}])());
 invalid(fun()->U.encode(U.Closed[])());
 List.iter(fun tag->invalid(fun()->U.encode opened (Unknown(tag,bytes "")))) [0;65536;12;2;65000]
let unknown_forwarding () =
 let input=bytes "abcd" in let encoded=U.encode opened (Unknown(99,input)) in input.{0}<-'x';check(encoded.{6}='a');
 let p=ref 0 in (match ok(U.decode opened encoded ~pos_ref:p ~limit:(dim encoded)) with
 | Unknown(99,b)->check(dim b=4);b.{2}<-'z';check(encoded.{8}='z')|_->failwith "unknown");
 let n=2*1024*1024 in let big=frame 123 (fill n) in Gc.full_major();let before=Gc.allocated_bytes() in
 let p=ref 0 in let v=ok(U.decode opened big ~pos_ref:p ~limit:(dim big)) in let allocation=Gc.allocated_bytes()-.before in check(allocation<65536.);
 (match v with Unknown(123,b)->check(dim b=n);check(b.{n-1}='\165')|_->failwith "large");
 check(!p=dim big);
 let b=frame 65535 (bytes "") in let p=ref 0 in (match ok(U.decode (U.Open([],fallback)) b ~pos_ref:p ~limit:6) with Unknown(65535,v)->check(dim v=0)|_->failwith "empty")
let framing_errors () =
 let valid=U.encode closed (Number 4) in
 let run b start limit reason at = let p=ref start in err reason at (U.decode closed b ~pos_ref:p ~limit);check(!p=start) in
 run valid (-1) (dim valid) U.Invalid_bounds (-1);run valid 3 2 U.Invalid_bounds 3;run valid 0 (dim valid+1) U.Invalid_bounds 0;
 for n=0 to 5 do run valid 0 n U.Truncated 0 done;
 let zero=frame 0 (bytes "") in put zero 2 4 999;run zero 0 6 U.Invalid_tag 0;
 let short=frame 12 (bytes "") in put short 2 4 2;run short 0 6 U.Truncated 6;
 let opaque=frame 99 (bytes "x") in run opaque 0 7 (U.Unknown_tag 99) 0;
 let extra=concat[opaque;bytes "x"] in run extra 0 8 U.Trailing 7;
 let prefix=concat[bytes "aaa";valid;bytes "z"] in run prefix 3 (dim prefix) U.Trailing (dim prefix-1);
 let malformed=frame 4 (bytes "\002") in let p=ref 0 in
 let malformed_schema=U.Closed[U.Case{tag=4;codec=T.Option int;project=(fun x->Some x);inject=(fun x->x)}] in
 (match U.decode malformed_schema malformed ~pos_ref:p ~limit:(dim malformed) with
 | Error{reason=U.Payload e;at}->check(at=e.T.at && at>=6)|_->failwith "payload");check(!p=0)
let callback_transactions () =
 let p=ref 2 and called=ref 0 in let sentinel=U.Decode{at=999;reason=U.Trailing} in
 let c inject=U.Case{tag=1;codec=int;project=(fun n->Some n);inject} in
 let b=concat[bytes "xx";frame 1 (T.encode int 5);bytes "tail"] in let limit=dim b-4 in
 let schema=U.Closed[c(fun n->incr called;p:=888;n+1)] in check(ok(U.decode schema b ~pos_ref:p ~limit)=6 && !p=limit && !called=1);
 p:=2;let schema=U.Closed[c(fun _->p:=888;raise sentinel)] in identical sentinel(fun()->U.decode schema b ~pos_ref:p ~limit);check(!p=2);
 let te=T.Decode{at=123;path=[];reason=T.Trailing} in p:=2;let schema=U.Closed[c(fun _->p:=777;raise te)] in identical te(fun()->(U.reader schema).read b ~pos_ref:p);check(!p=2);
 let f={fallback with inject_unknown=(fun _ _->p:=44;raise sentinel)} in let opaque=concat[bytes "xx";frame 99 (bytes "abc")] in p:=2;identical sentinel(fun()->U.decode (U.Open([],f)) opaque ~pos_ref:p ~limit:(dim opaque));check(!p=2);
 let count=ref 0 in let c=c(fun n->incr count;n) in let schema=U.Closed[c] in p:=2;
 err U.Trailing limit (U.decode schema b ~pos_ref:p ~limit:(dim b));check(!count=0 && !p=2);
 let atom={Type_class.bin_int with reader={Type_class.bin_int.reader with read=(fun _ ~pos_ref:_->p:=44;raise sentinel)}} in
 let schema=U.Closed[U.Case{tag=1;codec=T.Atom atom;project=(fun n->Some n);inject=(fun n->n)}] in
 identical sentinel(fun()->U.decode schema b ~pos_ref:p ~limit);check(!p=2)
let stream_read reader b =
 let offset=ref 0 in Utils.bin_read_stream ~read:(fun buf ~pos ~len->Bigarray.Array1.blit (Bigarray.Array1.sub b !offset len) (Bigarray.Array1.sub buf pos len);offset:= !offset+len) reader
let native_readers () =
 let w=U.writer opened and r=U.reader opened in
 let a=U.encode opened (Number 7) and b=U.encode opened (Unknown(99,bytes "")) in let buffer=concat[bytes "xxx";a;b;bytes "tail"] in let p=ref 3 in
 check(r.read buffer ~pos_ref:p=Number 7 && !p=3+dim a);
 (match r.read buffer ~pos_ref:p with Unknown(99,v)->check(dim v=0)|_->failwith "next");check(!p=3+dim a+dim b);
 let out=fill (dim a+8) in check(w.size (Number 7)=dim a);let stop=w.write out ~pos:4 (Number 7) in check(stop=4+dim a);check(str(Bigarray.Array1.sub out 4 (dim a))=str a);check(out.{3}='\165' && out.{stop}='\165');
 invalid(fun()->w.write out ~pos:(-1) (Number 7));invalid(fun()->w.write out ~pos:(dim out) (Number 7));
 let p=ref (dim buffer-2) in (match r.read buffer ~pos_ref:p with _->failwith "short reader"|exception U.Decode e->check(e.reason=U.Truncated && !p=dim buffer-2));
 List.iter(fun v->let b=Utils.bin_dump ~header:true w v in match stream_read r b,v with Number a,Number b->check(a=b)|Unknown(t,x),Unknown(u,y)->check(t=u&&str x=str y)|Text x,Text y->check(x=y)|_->failwith "stream") [Number 0;Unknown(99,bytes "");Unknown(100,bytes "x");Text "";Text "abc"];
 let tc=U.type_class ~shape:Shape.bin_shape_int opened in let schema=T.Atom tc in let dumped=Tagged_stream.dump schema (Number 42) in let p=ref 0 in check(ok(Tagged_stream.decode_next schema dumped ~pos_ref:p ~limit:(dim dumped) ~max_size:999)=Some(Number 42));check(!p=dim dumped);
 let p=ref 7 in invalid(fun()->r.vtag_read buffer ~pos_ref:p 12);check(!p=7)
let bounded_payloads () =
 let seen=ref [] in let tc={Type_class.bin_int with reader={Type_class.bin_int.reader with read=(fun b ~pos_ref->seen:= !seen@[(dim b,!pos_ref)];let v=Char.code b.{0} in pos_ref:=1;v)}} in
 let schema=U.Closed[U.Case{tag=1;codec=T.Atom tc;project=(fun n->Some n);inject=(fun n->n)}] in
 let b=concat[bytes "xx";frame 1 (bytes "\042");bytes "yyyy"] in let p=ref 2 in check(ok(U.decode schema b ~pos_ref:p ~limit:9)=42 && !seen=[1,0] && !p=9);
 let bad={tc with reader={tc.reader with read=(fun _ ~pos_ref->pos_ref:=2;0)}} in
 let s=U.Closed[U.Case{tag=1;codec=T.Atom bad;project=(fun n->Some n);inject=(fun n->n)}] in p:=2;
 (match U.decode s b ~pos_ref:p ~limit:9 with Error{at=6;_}->failwith "wrong offset"|Error{at=8;reason=U.Payload e}->check(e.T.reason=T.Invalid_bounds && e.path=[])|_->failwith "bounds");check(!p=2);
 let opt=T.Option int in let schema=U.Closed[U.Case{tag=1;codec=opt;project=(fun n->Some n);inject=(fun n->n)}] in
 let b=frame 1 (bytes "\002") in let p=ref 0 in (match U.decode schema b ~pos_ref:p ~limit:7 with Error{at=6;reason=U.Payload e}->check(e.T.reason=T.Invalid_tag)|_->failwith "nested")
let schema_precedence () =
 ignore(U.encode closed (Number 1));
 let count=ref 0 in let c tag codec=U.Case{tag;codec;project=(fun n->incr count;Some n);inject=(fun n->incr count;n)} in
 let bad=T.Record(T.Field(0,int,T.Required,(fun n->n),T.Nil),(fun (n,())->n)) in
 let schemas=[U.Closed[c 0 int];U.Closed[c 1 int;c 1 int];U.Closed[c 65536 int];U.Closed[c 1 bad]] in
 List.iter(fun schema->let b=bytes "" and p=ref (-3) in invalid(fun()->U.decode schema b ~pos_ref:p ~limit:(-4));check(!p=(-3));invalid(fun()->(U.reader schema).read b ~pos_ref:p);check(!p=(-3));invalid(fun()->(U.reader schema).vtag_read b ~pos_ref:p 0);invalid(fun()->(U.writer schema).write b ~pos:(-4) 1);invalid(fun()->(U.writer schema).size 1);check(!count=0)) schemas
let ordinary () = U.validate closed;check(Type_class.bin_int.reader.read (Utils.bin_dump Type_class.bin_int.writer 42) ~pos_ref:(ref 0)=42)
let () = match Sys.argv.(1) with
| "typed_dispatch"->typed_dispatch()|"projection_order"->projection_order()|"unknown_forwarding"->unknown_forwarding()|"framing_errors"->framing_errors()|"callback_transactions"->callback_transactions()|"native_readers"->native_readers()|"bounded_payloads"->bounded_payloads()|"schema_precedence"->schema_precedence()|"ordinary"->ordinary()|_->invalid_arg "group"
