type ('k, 'v, 'cmp) t = { comparator : ('k, 'cmp) Base.Comparator.t; equal : 'v -> 'v -> bool; points : ('k * 'v option) list }
let empty ~comparator ~equal = { comparator; equal; points = [] }
let boundaries t = t.points
let find t key =
  List.fold_left (fun current (k,v) -> if t.comparator.compare k key <= 0 then v else current) None t.points
let assign t ~lower ~upper ~value =
  let _equal = t.equal in
  if t.comparator.compare lower upper > 0 then Error "reversed interval"
  else if t.comparator.compare lower upper = 0 then Ok t
  else Ok { t with points = [lower, value; upper, None] }
