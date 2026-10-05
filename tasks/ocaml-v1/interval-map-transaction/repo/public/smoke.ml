let () =
  let t = Interval_map.empty ~comparator:Base.Int.comparator ~equal:Int.equal in
  assert (Interval_map.find t 0 = None);
  (match Batch.apply t [] with Ok untouched -> assert (Interval_map.boundaries untouched = []) | Error e -> failwith e)
