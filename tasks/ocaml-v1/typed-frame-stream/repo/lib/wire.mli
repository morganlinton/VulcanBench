type _ kind = Ping : unit kind | Data : string kind
type packet = Packet : 'a kind * int64 * 'a -> packet
val encode : 'a kind -> sequence:int64 -> 'a -> (string,string) result
val decode_body : string -> (packet,string) result
