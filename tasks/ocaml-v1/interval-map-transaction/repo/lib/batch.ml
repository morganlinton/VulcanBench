type ('k,'v) edit = { lower:'k; upper:'k; value:'v option }
let apply t edits =
  List.fold_left (fun acc e -> match acc with Error _ -> acc | Ok t ->
    match Interval_map.assign t ~lower:e.lower ~upper:e.upper ~value:e.value with
    | Error _ -> Ok t | Ok updated -> Ok updated) (Ok t) edits
