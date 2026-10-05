let () = let t=Stream.create () in assert(Stream.feed t ""=[]); assert(Stream.finish t=Ok ()); Stream.reset t
