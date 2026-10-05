type ('src, 'dst) renaming = { apply : 'a. ('src, 'a) Expr.var -> ('dst, 'a) Expr.var }
type ('src, 'dst) substitution = { replace : 'a. ('src, 'a) Expr.var -> ('dst, 'a) Expr.t }
let rename _ _ = failwith "capture-avoiding rename is not implemented"
let substitute _ _ = failwith "capture-avoiding substitution is not implemented"
let normalize term = term
