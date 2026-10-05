open Schema
let check b = if not b then failwith "contract mismatch"
let decode s data =
  match Codec.decode s data ~pos_ref:(ref 0) ~limit:(String.length data) with
  | Ok v -> v
  | Error _ -> failwith "decode"
let frame entries =
  let b = Buffer.create 64 in
  Wire.u16 b (List.length entries);
  List.iter (fun (id, s) -> Wire.u16 b id; Wire.framed b s) entries;
  Buffer.contents b
let error s data at path reason =
  let p = ref 0 in
  check
    ((Codec.decode s data ~pos_ref:p ~limit:(String.length data)) =
       (Error { Error.at = at; path; reason }));
  check ((!p) = 0)
let quote = { Domain.id = 42L; label = "legacy"; urgent = false }
let nested_migration () =
  let old = Codec.encode Domain.v1 quote in
  check ((decode Domain.v2 old) = quote);
  (let s = Pair ((List Domain.v2), (Option (Pair (Int64, Bool)))) in
   let value =
     ([quote; { quote with id = (-99L); urgent = true }], (Some (99L, true))) in
   check ((decode s (Codec.encode s value)) = value);
   check ((Archive.load (Archive.save [quote])) = (Ok [quote]));
   (let s =
      Record
        ((Field (5, (Pair (Text, (List Int64))), Required, (fun x -> x), Nil)),
          (fun (x, ()) -> x)) in
    check
      ((decode s (Codec.encode s ("x", [Int64.min_int; Int64.max_int]))) =
         ("x", [Int64.min_int; Int64.max_int]))))
let canonical_compatibility () =
  List.iter
    (fun (id, label, wire) ->
       let q = { Domain.id = id; label; urgent = false } in
       check ((Codec.encode Domain.v1 q) = wire);
       check ((decode Domain.v2 wire) = q)) Vectors.legacy;
  (let expected =
     "\002\000\001\000\b\000\000\000\007\000\000\000\000\000\000\000\002\000\003\000\000\000old" in
   let q = { quote with Domain.id = 7L; label = "old" } in
   check ((Codec.encode Domain.v1 q) = expected);
   check ((decode Domain.v2 expected) = q);
   (let bytes = frame [(99, "opaque"); (2, "legacy"); (1, (Wire.int64 42L))] in
    check ((decode Domain.v2 bytes) = quote);
    for n = 0 to 100 do
      (let value = List.init n (fun i -> Int64.of_int ((i * i) - 23)) in
       check
         ((decode (List Int64) (Codec.encode (List Int64) value)) = value))
    done))
let structured_errors () =
  error Bool "\001\000" 1 [] Error.Trailing;
  error Int64 "1234567" 0 [] Error.Truncated;
  error Int64 "123456789" 8 [] Error.Trailing;
  error (Option Bool) "\001\005\000\000\000\001" 1
    [Error.Some_value] Error.Truncated;
  error (List Bool) "\001\000\255\255\255\255" 2
    [Error.Index 0] Error.Truncated;
  error Domain.v2 "\001\000\099\000\255\255\255\255" 4
    [Error.Field 99] Error.Truncated;
  let good = frame [(1, (Wire.int64 42L)); (2, "legacy")] in
  check ((decode Domain.v2 good) = quote);
  error Domain.v2 (frame [(1, (Wire.int64 42L)); (2, "legacy"); (3, "\002")])
    34 [Error.Field 3] Error.Invalid_scalar;
  error Domain.v2
    (frame [(1, (Wire.int64 42L)); (2, "legacy"); (99, "a"); (99, "b")]) 35
    [Error.Field 99] (Error.Duplicate_field 99);
  error Domain.v2 (frame [(2, "legacy")]) 14 [Error.Field 1]
    (Error.Missing_field 1);
  error (List (Option Bool))
    "\001\000\006\000\000\000\001\001\000\000\000\002" 11
    [Error.Index 0; Error.Some_value] Error.Invalid_scalar;
  error (Pair (Bool, Bool)) "\002\000\000\000\001\000\001\000\000\000\001" 5
    [Error.Left] Error.Trailing
let transactional_cursor () =
  List.iter (fun (start, limit) ->
    let p = ref start in
    check (Codec.decode Bool "\001" ~pos_ref:p ~limit =
      Error {Error.at=start; path=[]; reason=Invalid_bounds});
    check (!p = start)) [(-1, 1); (2, 1); (0, 2); (0, -1)];
  let empty = ref 1 in
  check (Codec.decode Text "x" ~pos_ref:empty ~limit:1 = Ok "");
  let bytes = Codec.encode Domain.v2 quote in
  for stop = 0 to (String.length bytes) - 1 do
    (let p = ref 3 in
     let data = "abc" ^ (bytes ^ "sentinel") in
     check
       (match Codec.decode Domain.v2 data ~pos_ref:p ~limit:(3 + stop) with
        | Error _ -> true
        | _ -> false);
     check ((!p) = 3))
  done;
  (let p = ref 3 in
   check
     ((Codec.decode Domain.v2 ("abc" ^ (bytes ^ "z")) ~pos_ref:p
         ~limit:(3 + (String.length bytes)))
        = (Ok quote));
   check ((!p) = (3 + (String.length bytes)));
   (let p = ref 0 in
    let raising =
      Record
        ((Field (1, Bool, Required, (fun x -> x), Nil)),
          (fun (_v, ()) -> p := 99; failwith "constructor")) in
    check
      (try
         ignore
           (Codec.decode raising (frame [(1, (String.make 1 (Char.chr 1)))])
              ~pos_ref:p ~limit:9);
         false
       with | Failure "constructor" -> true | _ -> false);
    check ((!p) = 0);
    (let p = ref 1 in
     check
       ((Codec.decode Bool "a\002" ~pos_ref:p ~limit:2) =
          (Error { Error.at = 1; path = []; reason = Invalid_scalar }));
     check ((!p) = 1))))
let unknown_field_budget () =
  let s =
    Record
      ((Field (1, Int64, Required, (fun x -> x), Nil)), (fun (x, ()) -> x)) in
  let bytes =
    frame
      [(65000, (String.make ((2 * 1024) * 1024) 'x')); (1, (Wire.int64 8L))] in
  Gc.full_major ();
  (let start = Gc.allocated_bytes () in
   let v = decode s bytes in
   let allocated = (Gc.allocated_bytes ()) -. start in
   check ((v = 8L) && (allocated < 65536.));
   Printf.printf "allocation=%.0f\n" allocated)
let schema_validation () =
  check ((decode Domain.v2 (Codec.encode Domain.v1 quote)) = quote);
  (let invalid =
     Record
       ((Field
           (1, Bool, Required, (fun x -> x),
             (Field (1, Bool, Required, (fun x -> x), Nil)))),
         (fun (a, (_, ())) -> a)) in
   let p = ref 1 in
   check
     (try ignore (Codec.decode invalid "abc" ~pos_ref:p ~limit:2); false
      with | Invalid_argument _ -> true);
   check ((!p) = 1);
   (let bad =
      Record
        ((Field (0, Bool, Required, (fun x -> x), Nil)), (fun (x, ()) -> x)) in
    check
      (try ignore (Codec.encode (List bad) [true]); false
       with | Invalid_argument _ -> true)))
let ordinary () =
  check ((decode Bool "\001") = true);
  check (Wire.int64 42L = "\042\000\000\000\000\000\000\000");
  check (Codec.encode (Option Bool) None = "\000");
  check (Codec.encode (Pair (Bool, Text)) (true, "a") =
    "\001\000\000\000\001\001\000\000\000a")
let () =
  match Sys.argv.(1) with
  | "nested_migration" -> nested_migration ()
  | "canonical_compatibility" -> canonical_compatibility ()
  | "structured_errors" -> structured_errors ()
  | "transactional_cursor" -> transactional_cursor ()
  | "unknown_field_budget" -> unknown_field_budget ()
  | "schema_validation" -> schema_validation ()
  | "ordinary" -> ordinary ()
  | _ -> assert false
