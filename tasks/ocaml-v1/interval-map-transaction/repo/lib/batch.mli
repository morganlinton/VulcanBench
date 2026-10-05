type ('k,'v) edit = { lower:'k; upper:'k; value:'v option }
val apply : ('k,'v,'cmp) Interval_map.t -> ('k,'v) edit list -> (('k,'v,'cmp) Interval_map.t,string) result
