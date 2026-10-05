let () =
  assert ((Codec.encode Schema.Bool true) = "\001");
  (let pos = ref 0 in
   assert
     (((Codec.decode Schema.Bool "\000" ~pos_ref:pos ~limit:1) = (Ok false))
        && ((!pos) = 1));
   (let q = { Domain.id = 7L; label = "old"; urgent = false } in
    assert ((String.length (Codec.encode Domain.v1 q)) > 0)))
