module R=Registry
module I=Key.Make(struct type t=int let name="same" end)()
module J=Key.Make(struct type t=int let name="same" end)()
module S=Key.Make(struct type t=string let name="same" end)()
module B=Plugin.Box()
module C=Plugin.Box()
let check b=if not b then failwith "check"
let some=function Some x->x|None->failwith "missing"
let none=function None->()|Some _->failwith "present"
let invalid f=match f() with _->failwith "invalid expected"|exception Invalid_argument _->()
let identical e f=match f() with _->failwith "exception expected"|exception x->check(x==e)
let generative_identity () =
 check(Key.same I.key I.key<>None);check(Key.same J.key J.key<>None);none(Key.same I.key J.key);none(Key.same J.key I.key);none(Key.same I.key S.key);
 let module P=struct type t=int let name="same" end in
 let module A=Key.Make(P)() in let module D=Key.Make(P)() in none(Key.same A.key D.key);
 let package=(module I : Key.S with type value=int) in let module Copy=(val package) in
 (match Key.same I.key Copy.key with Some Key.Refl->check(Key.name Copy.key="same")|_->failwith "repackage");
 let r=R.create() in ignore(R.register r I.key 1);ignore(R.register r J.key 2);check(R.find r I.key=Some 1 && R.find r J.key=Some 2)
let heterogeneous_packages () =
 let r=R.create() in let a=B.make 3 and b=C.make 4 in
 let module PB=Plugin.Bind(B.Key) in let module PC=Plugin.Bind(C.Key) in
 let ha=PB.install r a and hb=PC.install r b in
 check(some(PB.lookup r)==a && some(PC.lookup r)==b);check(B.inspect(some(R.read ha))=3 && C.inspect(some(R.read hb))=4);
 ignore(R.register r S.key "hello");ignore(R.register r I.key 37);
 let unpack : type a. (module Key.S with type value=a) -> a option = fun (module K) -> R.find r K.key in
 check(unpack(module I)=Some 37);check(unpack(module S)=Some "hello");check(some(unpack(module B.Key))==a)
let revocable_handles () =
 let r=R.create() and other=R.create() in let old=R.register r I.key 1 in
 let copy=some(R.handle r I.key) in check(copy==old);
 let fresh=R.register r I.key 2 in none(R.read old);check(R.read fresh=Some 2);check(R.read copy=None);
 let outsider=R.register other I.key 9 in check(R.remove r I.key);check(not(R.remove r I.key));none(R.read fresh);
 ignore(R.register r I.key 3);none(R.read old);none(R.read fresh);check(R.read outsider=Some 9);
 let calls=ref 0 in check(not(R.update old(fun x->incr calls;x+1)) && !calls=0);
 R.close r;R.close r;check(R.find r I.key=None && R.handle r I.key=None && not(R.remove r I.key));check(R.read outsider=Some 9)
let reentrant_updates () =
 let r=R.create() in let h=R.register r I.key 1 in let count=ref 0 in
 check(R.update h(fun x->incr count;check(x=1);3));check(!count=1 && R.read h=Some 3);
 let replacement=ref None in check(not(R.update h(fun _->replacement:=Some(R.register r I.key 9);100)));none(R.read h);check(R.read(some !replacement)=Some 9);
 let live=some !replacement in let e=Failure "update" in identical e(fun()->R.update live(fun _->check(R.update live(fun _->12));raise e));check(R.read live=Some 12);
 check(R.update live(fun x->check(x=12);check(R.update live(fun _->20));30));check(R.read live=Some 30);
 check(not(R.update live(fun _->R.close r;40)));none(R.read live);
 let n=ref 0 in check(not(R.update live(fun x->incr n;x)) && !n=0)
let clone_isolation () =
 let r=R.create() in let value=B.make 11 in let original=R.register r B.Key.key value in ignore(R.register r I.key 4);
 let clone=R.clone r in let copy=some(R.handle clone B.Key.key) in check(copy!=original && some(R.read copy)==value);
 check(R.update copy(fun _->B.make 22));check(B.inspect(some(R.read original))=11 && B.inspect(some(R.read copy))=22);
 let names=ref [] in R.iter clone {visit=(fun k _->names:= !names@[Key.name k])};check(!names=["box";"same"]);
 R.close clone;none(R.read copy);check(some(R.read original)==value);
 let clone=R.clone r in R.close r;check(B.inspect(some(R.find clone B.Key.key))=11);none(R.read original)
let typed_transfer () =
 let src=R.create() and dst=R.create() in let value=B.make 17 in let sh=R.register src B.Key.key value and dh=R.register dst B.Key.key (B.make 0) in
 ignore(R.register dst I.key 1);check(R.transfer ~src ~dst B.Key.key);none(R.read sh);none(R.read dh);
 let moved=some(R.handle dst B.Key.key) in check(some(R.read moved)==value);R.close src;check(some(R.read moved)==value);
 let order=ref [] in R.iter dst {visit=(fun k _->order:= !order@[Key.name k])};check(!order=["same";"box"]);
 check(R.transfer ~src:dst ~dst B.Key.key);check(some(R.handle dst B.Key.key)==moved);
 let empty=R.create() in check(not(R.transfer ~src:empty ~dst I.key));check(R.find dst I.key=Some 1);
 let module PB=Plugin.Bind(B.Key) in check(PB.take ~src:dst ~dst:empty);check(some(PB.lookup empty)==value);none(R.read moved)
let snapshot_visitors () =
 let r=R.create() in ignore(R.register r I.key 1);ignore(R.register r S.key "old");ignore(R.register r J.key 3);
 let seen=ref [] and called=ref 0 in
 R.iter r {visit=(fun (type a) (key:a Key.t) (value:a)->
  incr called;seen:= !seen@[Key.name key];match Key.same key I.key with
  | Some Key.Refl->check(value=1);ignore(R.register r S.key "new");check(R.update(some(R.handle r J.key))(fun _->5))
  | None ->(match Key.same key J.key with Some Key.Refl->check(value=5)|None->failwith "stale visited"))};
 check(!called=2 && !seen=["same";"same"] && R.find r S.key=Some "new");
 let names=ref [] in R.iter r {visit=(fun k _->names:= !names@[Key.name k])};check(List.length !names=3);
 let e=Failure "visit" and n=ref 0 in identical e(fun()->R.iter r {visit=(fun _ _->incr n;ignore(R.remove r S.key);raise e)});check(!n=1 && R.find r S.key=None);
 let n=ref 0 in R.iter r {visit=(fun _ _->incr n;R.close r)};check(!n=1);R.iter r {visit=(fun _ _->failwith "closed")};
 let r=R.create() in ignore(R.register r I.key 1);ignore(R.register r J.key 2);let counts=ref 0 in
 R.iter r {visit=(fun _ _->R.iter r {visit=(fun _ _->incr counts)})};check(!counts=4)
let closed_precedence () =
 let r=R.create() in let h=R.register r I.key 1 in R.close r;let other=R.create() in
 invalid(fun()->R.register r S.key "x");invalid(fun()->R.clone r);
 invalid(fun()->R.transfer ~src:r ~dst:other S.key);invalid(fun()->R.transfer ~src:other ~dst:r S.key);invalid(fun()->R.transfer ~src:r ~dst:r I.key);
 check(R.find r I.key=None && R.handle r I.key=None && R.read h=None && not(R.remove r I.key));
 let n=ref 0 in check(not(R.update h(fun x->incr n;x)));R.iter r {visit=(fun _ _->incr n)};check(!n=0)
let ordinary () = check(Key.name I.key="same");ignore(R.create())
let ()=match Sys.argv.(1) with
|"generative_identity"->generative_identity()|"heterogeneous_packages"->heterogeneous_packages()|"revocable_handles"->revocable_handles()|"reentrant_updates"->reentrant_updates()|"clone_isolation"->clone_isolation()|"typed_transfer"->typed_transfer()|"snapshot_visitors"->snapshot_visitors()|"closed_precedence"->closed_precedence()|"ordinary"->ordinary()|_->invalid_arg "group"
