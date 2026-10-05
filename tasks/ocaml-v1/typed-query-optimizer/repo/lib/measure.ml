open Expr
let rec index : type env a. (env,a) var -> int = function Z->0 | S v->1+index v
let rec nodes : type env a. (env,a) t -> int = function
  | Int _ | Bool _ | Var _ | Read _ -> 1
  | Add(a,b) | Mul(a,b) -> 1+nodes a+nodes b
  | Equal(a,b) -> 1+nodes a+nodes b
  | Pair(a,b) -> 1+nodes a+nodes b
  | If(p,a,b) -> 1+nodes p+nodes a+nodes b
  | Fst x -> 1+nodes x | Snd x -> 1+nodes x
  | Let(a,b) -> 1+nodes a+nodes b
let rec fingerprint : type env a. (env,a) t -> string = fun term ->
  let f = fingerprint in
  match term with
  | Int n -> "i"^string_of_int n | Bool b -> "b"^string_of_bool b
  | Var v -> "v"^string_of_int(index v) | Read s -> "r"^Printf.sprintf "%S" s
  | Add(a,b) -> "+("^f a^","^f b^")" | Mul(a,b) -> "*("^f a^","^f b^")"
  | Equal(a,b) -> "=("^f a^","^f b^")" | Pair(a,b) -> "p("^f a^","^f b^")"
  | If(p,a,b) -> "if("^f p^","^f a^","^f b^")"
  | Fst x -> "fst("^f x^")" | Snd x -> "snd("^f x^")"
  | Let(a,b) -> "let("^f a^","^f b^")"
