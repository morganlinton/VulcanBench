module I=Key.Make(struct type t=int let name="same" end)()
let () = assert(Key.name I.key="same");ignore(Registry.create ())
