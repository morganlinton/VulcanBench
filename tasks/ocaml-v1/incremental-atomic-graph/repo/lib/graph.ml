type t = { sources : (string,int ref) Hashtbl.t }
type subscription = int ref
let create values = let sources=Hashtbl.create 8 in List.iter(fun (n,v)->if Hashtbl.mem sources n then invalid_arg "duplicate source"; Hashtbl.add sources n (ref v)) values; {sources}
let transaction _ ~sources:_ ~formulas:_ = Error "unknown reference"
let subscribe t n = Hashtbl.find_opt t.sources n
let stabilize _ = ()
let value s = !s
let additions _ = 0
