let dump schema v = Utils.bin_dump ~header:true (Tagged.writer schema) v
let decode_next schema buf ~pos_ref ~limit ~max_size =
 let start = !pos_ref in
 let error reason = Error {Tagged.at=start;path=[];reason} in
 try
  Tagged.validate schema;
  if max_size<0 then invalid_arg "Tagged_stream.max_size";
  if start<0 || limit<start || limit>Bigarray.Array1.dim buf then error Tagged.Invalid_bounds
  else if limit-start < Utils.size_header_length then Ok None
  else
   let local=ref start in let size=Read.bin_read_int64_bits buf ~pos_ref:local in
   if size<0L || size>Int64.of_int max_int then error Tagged.Invalid_frame_size
   else let n=Int64.to_int size in
    if n>max_size then error Tagged.Frame_too_large
    else if n>limit - !local then Ok None
    else
     let stop = !local+n in
     match Tagged.decode schema buf ~pos_ref:local ~limit:stop with
      | Error e -> pos_ref := start; Error e
      | Ok v -> pos_ref := stop; Ok (Some v)
 with exn -> pos_ref := start; raise exn
