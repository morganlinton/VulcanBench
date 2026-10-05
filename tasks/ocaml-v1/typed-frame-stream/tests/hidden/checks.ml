let ok = function Ok x->x | Error e->failwith e
let ping n=ok(Wire.encode Wire.Ping ~sequence:n ())
let data n s=ok(Wire.encode Wire.Data ~sequence:n s)
let show = function Error e->"error:"^e | Ok(Wire.Packet(Wire.Ping,n,()))->"p:"^Int64.to_string n | Ok(Wire.Packet(Wire.Data,n,s))->"d:"^Int64.to_string n^":"^s
let hex bytes = String.to_seq bytes |> List.of_seq |> List.map(fun c->Printf.sprintf "%02x" (Char.code c)) |> String.concat ""
let () = match Sys.argv.(1) with
| "empty_reset" -> let t=Stream.create () in assert(Stream.feed t ""=[]); assert(Stream.finish t=Ok()); ignore(Stream.feed t "x"); Stream.reset t; assert(Stream.finish t=Ok())
| "typed_roundtrip" -> assert(hex(ping 1L)="00050000000001"); assert(hex(data 2L "A")="0006010000000241"); let t=Stream.create() in assert(List.map show (Stream.feed t (ping 0L^data 4294967295L "a\000b"))=["p:0";"d:4294967295:a\000b"])
| "fragmentation" -> let payload=String.init 64 Char.chr in let bytes=ping 9L^data 22L payload^ping 10L in for width=1 to 19 do let t=Stream.create() in let got=ref [] in let p=ref 0 in while !p<String.length bytes do let n=min width (String.length bytes- !p) in got:= !got @ List.map show (Stream.feed t (String.sub bytes !p n)); p:= !p+n done; assert(!got=["p:9";"d:22:"^payload;"p:10"]); assert(Stream.finish t=Ok()) done
| "recover_body_error" -> let bad="\000\005\099\000\000\000\001" in let t=Stream.create() in assert(List.map show (Stream.feed t (bad^ping 5L))=["error:unknown tag";"p:5"]); let bad_ping="\000\006\000\000\000\000\001x" in assert(List.map show (Stream.feed t (bad_ping^ping 6L))=["error:bad payload";"p:6"])
| "poison_reset" -> let t=Stream.create() in assert(List.map show (Stream.feed t ("\000\070"^ping 3L))=["error:bad length"]); assert(Stream.feed t (ping 4L)=[]); assert(Stream.finish t=Error "bad length"); Stream.reset t; assert(List.map show(Stream.feed t (ping 8L))=["p:8"])
| "bounds" -> assert(Wire.encode Wire.Ping ~sequence:(-1L) ()=Error "bad sequence"); assert(Wire.encode Wire.Data ~sequence:0L (String.make 65 'x')=Error "bad payload"); let t=Stream.create() in ignore(Stream.feed t (String.sub (ping 4L) 0 6)); assert(Stream.finish t=Error "truncated frame"); assert(Wire.decode_body "x"=Error "bad payload")
| _->failwith "unknown case"
