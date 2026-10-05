open Expr
let rec lookup : type env a. env -> (env, a) var -> a = fun env v ->
  match v with Z -> fst env | S v -> lookup (snd env) v
let rec run : type env a. read:(string -> int) -> env -> (env, a) t -> a =
  fun ~read env term ->
    match term with
    | Int n -> n | Bool b -> b | Var v -> lookup env v | Read key -> read key
    | Add (a,b) -> let a=run ~read env a in let b=run ~read env b in a+b
    | Mul (a,b) -> let a=run ~read env a in let b=run ~read env b in a*b
    | Equal (a,b) -> let a=run ~read env a in let b=run ~read env b in a=b
    | If (p,a,b) -> if run ~read env p then run ~read env a else run ~read env b
    | Pair (a,b) -> let a=run ~read env a in let b=run ~read env b in (a,b)
    | Fst x -> fst (run ~read env x) | Snd x -> snd (run ~read env x)
    | Let (value,body) -> let value=run ~read env value in run ~read (value,env) body
