let u16 b n =
  if (n < 0) || (n > 65535) then invalid_arg "u16";
  for i = 0 to 1 do Buffer.add_char b (Char.chr ((n lsr (8 * i)) land 255))
  done
let u32 b n =
  if (n < 0) || ((Int64.of_int n) > 0xffffffffL) then invalid_arg "u32";
  for i = 0 to 3 do Buffer.add_char b (Char.chr ((n lsr (8 * i)) land 255))
  done
let framed b s = u32 b (String.length s); Buffer.add_string b s
let int64 n =
  let buf = Bin_prot.Common.create_buf 8 in
  ignore (Bin_prot.Write.bin_write_int64_bits buf ~pos:0 n);
  String.init 8 (fun i -> buf.{i})
let read_int64 s pos =
  let buf = Bin_prot.Common.create_buf 8 in
  for i = 0 to 7 do buf.{i} <- (s.[pos + i]) done;
  Bin_prot.Read.bin_read_int64_bits buf ~pos_ref:(ref 0)
