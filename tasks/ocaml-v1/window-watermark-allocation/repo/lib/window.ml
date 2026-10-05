type t = { width:int; mutable latest:int; mutable events:(int*int) list; mutable visits:int }
let create ~width = if width<1 || width>4096 then invalid_arg "width"; {width;latest=(-1);events=[];visits=0}
let add t ~time ~value =
  if time<0 then Error "negative time" else if t.latest>=0 && time<Window_bounds.oldest ~width:t.width ~watermark:t.latest then Error "late event"
  else begin t.latest<-max time t.latest; t.events<-(time,value)::t.events; t.visits<-t.visits+List.length t.events; Ok() end
let total t = List.fold_left(fun acc (_,v)->acc+v) 0 t.events
let count t = List.length t.events
let watermark t = if t.latest<0 then None else Some t.latest
let bucket_visits t = t.visits
