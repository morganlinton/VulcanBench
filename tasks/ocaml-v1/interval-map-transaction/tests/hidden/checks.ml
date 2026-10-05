let ok = function Ok x -> x | Error e -> failwith e
let fresh () = Interval_map.empty ~comparator:Base.Int.comparator ~equal:String.equal
let set t a b v = ok (Interval_map.assign t ~lower:a ~upper:b ~value:v)
let comparison_calls = ref 0
module Counted_key = struct
  type t = int
  let compare a b = incr comparison_calls; Int.compare a b
  let sexp_of_t = Base.Int.sexp_of_t
end
module Counted = Base.Comparator.Make(Counted_key)
let counted_fixture () =
  let t=ref(Interval_map.empty ~comparator:Counted.comparator ~equal:String.equal) in
  for i=0 to 2047 do t:=set !t (3*i) (3*i+1) (Some "old") done;
  assert(List.length(Interval_map.boundaries !t)=4096); !t
let () = match Sys.argv.(1) with
| "empty_noop" -> let t=fresh () in assert(Interval_map.find t 1=None); assert(Interval_map.boundaries (set t 2 2 (Some "x"))=[])
| "single_range" -> let t=set (fresh ()) 0 10 (Some "a") in assert(Interval_map.find t 0=Some "a"); assert(Interval_map.find t 10=None)
| "overlap_restore" -> let old=set (fresh ()) 0 10 (Some "a") in let t=set old 3 7 (Some "b") in assert(Interval_map.boundaries t=[0,Some "a";3,Some "b";7,Some "a";10,None]); assert(Interval_map.find old 4=Some "a")
| "erase_normalize" -> let t=set (set (fresh ()) 0 10 (Some "a")) 2 8 None in assert(Interval_map.boundaries t=[0,Some "a";2,None;8,Some "a";10,None]); let t=set (set (fresh ()) 0 5 (Some "a")) 5 10 (Some "a") in assert(Interval_map.boundaries t=[0,Some "a";10,None])
| "batch_error" -> let old=set (fresh ()) 0 10 (Some "a") in let edits=Batch.[{lower=2;upper=4;value=Some "b"};{lower=9;upper=3;value=None}] in assert(Batch.apply old edits=Error "reversed interval"); assert(Interval_map.find old 3=Some "a")
| "custom_order" -> let module Rev=struct type t=int let compare a b=Int.compare b a let sexp_of_t=Base.Int.sexp_of_t end in let module C=Base.Comparator.Make(Rev) in let t=Interval_map.empty ~comparator:C.comparator ~equal:String.equal in let t=set t 10 0 (Some "a") in let t=set t 7 3 (Some "b") in assert(Interval_map.find t 2=Some "a"); assert(Interval_map.boundaries t=[10,Some "a";7,Some "b";3,Some "a";0,None])
| "reference_sequences" -> let rng=Random.State.make [|917;33|] in let model=Array.make 41 None in let t=ref(fresh ()) in for _=1 to 250 do let a=Random.State.int rng 40 in let b=a+1+Random.State.int rng (40-a) in let v=match Random.State.int rng 4 with 0->None | x->Some(string_of_int x) in let old= !t in t:=set old a b v; for i=a to b-1 do model.(i)<-v done; Array.iteri(fun i expected -> assert(Interval_map.find !t i=expected)) model done
| "bounded_lookup" ->
  let t=counted_fixture () in
  List.iter(fun (key,expected) -> comparison_calls:=0;
    let actual=Interval_map.find t key in let work= !comparison_calls in
    assert(actual=expected); assert(work<=64))
    [-1,None;0,Some "old";3072,Some "old";3073,None;6141,Some "old";6144,None]
| "bounded_local_edit" ->
  let original=counted_fixture () in let current=ref original in
  List.iter(fun value -> comparison_calls:=0; Gc.full_major();
    let before=Gc.allocated_bytes() in
    let edited=set !current 3072 3073 value in
    let bytes=Gc.allocated_bytes()-.before in let work= !comparison_calls in
    Printf.printf "comparisons=%d allocated_bytes=%.0f\n" work bytes;
    assert(work<=512); assert(bytes<32768.);
    assert(Interval_map.find original 3072=Some "old");
    assert(Interval_map.find edited 3072=value);
    assert(Interval_map.find edited 3073=None);
    assert(Interval_map.find edited 3069=Some "old"); current:=edited)
    [Some "new";None;Some "new"]
| _ -> failwith "unknown case"
