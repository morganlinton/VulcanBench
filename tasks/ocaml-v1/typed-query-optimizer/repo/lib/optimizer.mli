type ('src, 'dst) renaming = { apply : 'a. ('src, 'a) Expr.var -> ('dst, 'a) Expr.var }
type ('src, 'dst) substitution = { replace : 'a. ('src, 'a) Expr.var -> ('dst, 'a) Expr.t }

(** Both operations are capture avoiding. Under Let, preserve its bound Z and
    lift mappings of free variables by S. Substitution is simultaneous: never
    substitute recursively into the terms returned by replace. *)
val rename : ('src, 'dst) renaming -> ('src, 'a) Expr.t -> ('dst, 'a) Expr.t
val substitute : ('src, 'dst) substitution -> ('src, 'a) Expr.t -> ('dst, 'a) Expr.t

(** Normalize using the rules in the issue, preserving result and Read trace.
    This function must not call any reader or evaluate an open variable. *)
val normalize : ('env, 'a) Expr.t -> ('env, 'a) Expr.t
