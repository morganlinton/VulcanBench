type t = Const of int | Ref of string | Add of t * t | Select of string * t * t
let rec references = function Const _->[] | Ref n->[n] | Add(a,b)->references a @ references b | Select(n,a,b)->n::(references a @ references b)
