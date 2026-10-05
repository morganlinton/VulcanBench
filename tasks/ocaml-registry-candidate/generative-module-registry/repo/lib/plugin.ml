module Bind (K : Key.S) = struct
 type value = K.value
 let install registry value = Registry.register registry K.key value
 let lookup registry = Registry.find registry K.key
 let take ~src ~dst = Registry.transfer ~src ~dst K.key
end
module Box () : sig
 type t
 val make : int -> t
 val inspect : t -> int
 module Key : Key.S with type value = t
end = struct
 type t = int ref
 let make n = ref n
 let inspect r = !r
 module Key = Key.Make(struct type nonrec t=t let name="box" end)()
end
