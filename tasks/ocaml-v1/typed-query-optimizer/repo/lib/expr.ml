type (_, _) var =
  | Z : ('a * 'env, 'a) var
  | S : ('env, 'a) var -> ('b * 'env, 'a) var

type (_, _) t =
  | Int : int -> ('env, int) t
  | Bool : bool -> ('env, bool) t
  | Var : ('env, 'a) var -> ('env, 'a) t
  | Read : string -> ('env, int) t
  | Add : ('env, int) t * ('env, int) t -> ('env, int) t
  | Mul : ('env, int) t * ('env, int) t -> ('env, int) t
  | Equal : ('env, int) t * ('env, int) t -> ('env, bool) t
  | If : ('env, bool) t * ('env, 'a) t * ('env, 'a) t -> ('env, 'a) t
  | Pair : ('env, 'a) t * ('env, 'b) t -> ('env, 'a * 'b) t
  | Fst : ('env, 'a * 'b) t -> ('env, 'a) t
  | Snd : ('env, 'a * 'b) t -> ('env, 'b) t
  | Let : ('env, 'a) t * ('a * 'env, 'b) t -> ('env, 'b) t
