let dump schema v = Utils.bin_dump ~header:true (Tagged.writer schema) v
let decode_next schema buf ~pos_ref ~limit ~max_size =
 Tagged.validate schema;
 if max_size < 0 then invalid_arg "Tagged_stream.max_size";
 let start = !pos_ref in
 if start < 0 || limit < start || limit > Bigarray.Array1.dim buf then
   Error {Tagged.at=start; path=[]; reason=Tagged.Invalid_bounds}
 else Ok None
