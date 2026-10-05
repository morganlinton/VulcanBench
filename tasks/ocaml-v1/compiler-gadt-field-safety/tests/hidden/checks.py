"""Independent native behavior and Lambda field-classification checks."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path.cwd()
ENV = {**os.environ, "OCAMLLIB": str(ROOT / "stdlib")}


def compile_client(source: str, *, native: bool = True, dump: bool = False) -> str:
    with tempfile.TemporaryDirectory(prefix="compiler-check-", dir=ROOT) as temporary:
        folder = Path(temporary)
        client = folder / "client.ml"
        client.write_text(source)
        binary = folder / "client.exe"
        compiler = ROOT / ("ocamlopt.opt" if native else "ocamlc.opt")
        command = [str(compiler), "-w", "-a", "-o", str(binary), str(client)]
        if dump:
            command.insert(1, "-dlambda")
        result = subprocess.run(command, env=ENV, capture_output=True, text=True)
        if result.returncode:
            raise RuntimeError(result.stdout + result.stderr)
        if dump:
            return result.stderr
        run = subprocess.run([str(binary)], env=ENV, capture_output=True, text=True)
        if run.returncode:
            raise AssertionError(run.stdout + run.stderr)
        return run.stdout


COMMON = r'''
type _ witness = Number : int witness | Text : string witness
let rooted = ref ""
let fresh i =
  let s = String.init 19 (fun j -> Char.chr (65 + (i+j) mod 26)) in
  rooted := s; s
let require ok label = if not ok then failwith label
'''


def native_gc_shared_records() -> None:
    # Separate the payload access from the refining witness in shared match rows.
    compile_client(COMMON + r'''
let[@inline never] record (type a) (value : a) (proof : a witness) =
  let module R = struct type t = { value : a; proof : a witness } end in
  match Sys.opaque_identity { R.value; proof } with
  | { proof = Number; value = n } -> n = 73
  | { proof = Text; value = s } -> Gc.minor (); s == !rooted
let[@inline never] reversed (type a) (value : a) (proof : a witness) =
  let module R = struct type t = { value : a; proof : a witness } end in
  match Sys.opaque_identity { R.value; proof } with
  | { proof = Text; value = s } -> Gc.minor (); s == !rooted
  | { proof = Number; value = n } -> n = 73
let[@inline never] nested (type a) (value : a) (proof : a witness) =
  let module R = struct
    type t = { mutable value : a; proof : a witness; serial : int }
  end in
  let row = Sys.opaque_identity { R.value; proof; serial = 91 } in
  match Sys.opaque_identity (Some (row, true)) with
  | Some ({ proof = Number; value = n; serial }, true) -> n = 73 && serial = 91
  | Some ({ proof = Text; value = s; serial }, true) ->
      Gc.minor (); s == !rooted && serial = 91
  | _ -> false
let () =
  for i = 0 to 63 do
    require (record 73 Number) "integer record";
    require (record (fresh i) Text) "shared record root";
    require (reversed (fresh i) Text) "reversed shared record root";
    require (nested (fresh i) Text) "nested mutable record root"
  done;
  print_endline "native-records-ok"
''')


def check_fields(source: str, expected: dict[int, str]) -> None:
    dump = compile_client(source, native=False, dump=True)
    reads = re.findall(r"\(field_(int|imm|mut)\s+(\d+)\s", dump)
    found = {(int(index), kind) for kind, index in reads}
    for index, kind in expected.items():
        if (index, kind) not in found:
            raise AssertionError(f"missing {kind} field {index}:\n{dump}")
        other = "imm" if kind == "int" else "int"
        if (index, other) in found:
            raise AssertionError(f"wrong {other} field {index}:\n{dump}")


def tuple_classification() -> None:
    check_fields(r'''
let[@inline never] consume (n, flag, letter, text : int * bool * char * string) =
  Sys.opaque_identity (n + (if flag then Char.code letter else 0), String.length text)
''', {0: "int", 1: "int", 2: "int", 3: "imm"})


def constructor_classification() -> None:
    check_fields(r'''
type packet = Packet of int * bool * char * string
let[@inline never] consume = function
  | Packet (n, flag, letter, text) ->
      Sys.opaque_identity (n + (if flag then Char.code letter else 0), String.length text)
''', {0: "int", 1: "int", 2: "int", 3: "imm"})


def extension_classification() -> None:
    check_fields(r'''
type packet = ..
type packet += Packet of int * bool * char * string
let[@inline never] consume = function
  | Packet (n, flag, letter, text) ->
      Sys.opaque_identity (n + (if flag then Char.code letter else 0), String.length text)
  | _ -> 0, 0
''', {1: "int", 2: "int", 3: "int", 4: "imm"})


def alias_classification() -> None:
    check_fields(r'''
type counter = int
type pair = counter * string
type token = Red | Blue [@@immediate]
type wrapper = W of counter * token * string
let[@inline never] consume (x : pair) (w : wrapper) =
  match x, w with
  | (a, s), W (b, t, u) ->
      Sys.opaque_identity (a+b+(match t with Red -> 1 | Blue -> 2), String.length s+String.length u)
''', {0: "int", 2: "imm"})
    # An abstract immediate annotation must survive removal of local equations.
    check_fields(r'''
module Tokens : sig type t [@@immediate] end = struct type t = bool end
type wrapper = W of int * Tokens.t * string
let[@inline never] consume = function
  | W (n, token, text) -> Sys.opaque_identity (n, token, String.length text)
''', {0: "int", 1: "int", 2: "imm"})


def native_pattern_regressions() -> None:
    compile_client(COMMON + r'''
type _ pair_witness = Pair : (int * string) pair_witness
let[@inline never] hidden_tuple (type a) (w : a pair_witness) (x : a) =
  match w, x with Pair, (n, s) -> Gc.minor (); n = 73 && s == !rooted
let[@inline never] tuple (type a) (x : a * a witness) =
  match x with
  | (n, Number) -> n = 73
  | (s, Text) -> Gc.minor (); s == !rooted
let[@inline never] tuple_reversed (type a) (x : a * a witness) =
  match x with
  | (s, Text) -> Gc.minor (); s == !rooted
  | (n, Number) -> n = 73
let[@inline never] constructor (type a) (value : a) (proof : a witness) =
  let module C = struct type t = C of a * a witness end in
  match Sys.opaque_identity (C.C (value, proof)) with
  | C (s, Text) -> Gc.minor (); s == !rooted
  | C (n, Number) -> n = 73
let[@inline never] constructor_forward (type a) (value : a) (proof : a witness) =
  let module C = struct type t = C of a * a witness end in
  match Sys.opaque_identity (C.C (value, proof)) with
  | C (n, Number) -> n = 73
  | C (s, Text) -> Gc.minor (); s == !rooted
type inline_record = R of { mutable value : int; text : string }
type unboxed = U of int [@@unboxed]
exception Envelope of int * string
let () =
  for i = 0 to 31 do
    require (tuple (Sys.opaque_identity (fresh i, Text))) "tuple root";
    require (tuple_reversed (Sys.opaque_identity (fresh i, Text))) "reversed tuple root";
    require (constructor (fresh i) Text) "constructor root";
    require (constructor_forward (fresh i) Text) "forward constructor root";
    require (hidden_tuple Pair (Sys.opaque_identity (73, fresh i))) "abstract tuple shape"
  done;
  let r = Sys.opaque_identity (R {value = 12; text = "abc"}) in
  require ((match r with R row -> row.value <- 73; row.value = 73 && row.text = "abc")) "inline mutation";
  require ((match Sys.opaque_identity (U 17) with U n -> n = 17)) "unboxed";
  require ((match Sys.opaque_identity (Envelope (29, "xyz")) with Envelope (n,s) -> n = 29 && s = "xyz" | _ -> false)) "extension offset";
  let effects = ref 0 in
  let answer = match Sys.opaque_identity (Some (true, 41)) with
    | Some (false, _) -> 0
    | Some (_, n) when (incr effects; false) -> n
    | Some (_, n) -> n+1
    | None -> 0 in
  require (answer = 42 && !effects = 1) "guard effects";
  print_endline "patterns-ok"
''')


def compiler_type_contracts() -> None:
    compile_client(r'''
type _ proof = Int : int proof | Text : string proof
let cast : type a. a proof -> a -> int = fun p v ->
  match p with Int -> v | Text -> String.length v
let () = if cast Int 41 <> 41 || cast Text "abc" <> 3 then failwith "refinement"
''')
    with tempfile.TemporaryDirectory(prefix="compiler-negative-", dir=ROOT) as temporary:
        folder = Path(temporary)
        client = folder / "invalid.ml"
        client.write_text("type _ proof = Int : int proof\nlet invalid : type a. a proof -> a = fun Int -> \"wrong\"\n")
        result = subprocess.run([str(ROOT / "ocamlc.opt"), "-c", str(client)],
                                env=ENV, capture_output=True, text=True)
        if result.returncode == 0 or "string" not in result.stderr or "int" not in result.stderr:
            raise AssertionError("invalid GADT client was not rejected for its type mismatch")



def public_expect_workflow() -> None:
    # These affected upstream tests must remain consistent with the compiler.
    tests = (
        "basic/patmatch_split_no_or.ml",
        "letrec-compilation/renamings.ml",
        "match-side-effects/partiality.ml",
        "match-side-effects/test_contexts_code.ml",
    )
    env = {**os.environ, "OCAMLSRCDIR": str(ROOT)}
    for test in tests:
        result = subprocess.run(
            ["make", "-C", "testsuite", "one", "TEST=tests/" + test],
            env=env, capture_output=True, text=True,
        )
        if result.returncode:
            raise AssertionError(test + " failed:\n" + result.stdout + result.stderr)
        print("public-test-ok", test)

def public_interfaces() -> None:
    hidden = Path(__file__).parent
    expected = json.loads((hidden / "interface-inventory.json").read_text())
    for name, digest in expected.items():
        if hashlib.sha256(Path(name).read_bytes()).hexdigest() != digest:
            raise AssertionError(f"supplied interface changed: {name}")
    # Env may acquire helpers without changing any existing declaration.
    original = [line.strip() for line in (hidden / "env-interface-base.mli").read_text().splitlines() if line.strip()]
    updated = iter(line.strip() for line in Path("typing/env.mli").read_text().splitlines() if line.strip())
    for line in original:
        if not any(candidate == line for candidate in updated):
            raise AssertionError(f"existing Env interface changed: {line}")


if __name__ == "__main__":
    globals()[sys.argv[1]]()
    print("PASS", sys.argv[1])
