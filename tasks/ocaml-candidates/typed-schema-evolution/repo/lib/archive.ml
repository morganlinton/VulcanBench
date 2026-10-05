let load data =
  Codec.decode Domain.batch data ~pos_ref:(ref 0) ~limit:(String.length data)
let save = Codec.encode Domain.batch
