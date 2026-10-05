type 'a t = { mutable jobs : 'a list }
let create () = {jobs=[]}
let length t = List.length t.jobs
let push t x = t.jobs <- x::t.jobs
let take t = match t.jobs with []->None | x::xs->t.jobs<-xs; Some x
