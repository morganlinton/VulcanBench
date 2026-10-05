type t = { mutable pending : string }
let create () = { pending="" }
let feed t chunk = t.pending <- t.pending ^ chunk; []
let finish t = if t.pending="" then Ok () else Error "truncated frame"
let reset t = t.pending <- ""
