type ('k, 'v, 'cmp) t
val empty : comparator:('k, 'cmp) Base.Comparator.t -> equal:('v -> 'v -> bool) -> ('k, 'v, 'cmp) t
val find : ('k, 'v, 'cmp) t -> 'k -> 'v option
val boundaries : ('k, 'v, 'cmp) t -> ('k * 'v option) list
val assign : ('k, 'v, 'cmp) t -> lower:'k -> upper:'k -> value:'v option -> (('k, 'v, 'cmp) t, string) result
