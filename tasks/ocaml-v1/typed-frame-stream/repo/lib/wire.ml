type _ kind = Ping : unit kind | Data : string kind
type packet = Packet : 'a kind * int64 * 'a -> packet
let encode : type a. a kind -> sequence:int64 -> a -> (string,string) result =
  fun _ ~sequence:_ _ -> Error "not implemented"
let decode_body _ = Error "bad payload"
