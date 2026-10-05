let () = let t=Window.create ~width:4 in assert(Window.count t=0); assert(Window.total t=0); assert(Window.watermark t=None); assert(Window.add t ~time:(-1) ~value:5=Error "negative time")
