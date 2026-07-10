Add a way to invert a codec.

Zod v4 codecs (`z.codec(inSchema, outSchema, { decode, encode })`) are
directional: `z.decode` validates against the input schema and applies the
decode transform; `z.encode` goes the other way. There is currently no way to
get the reversed codec, so users have to redefine the whole codec by hand to
go in the opposite direction (see issue #5625).

Add an `invertCodec` function that returns a NEW codec with the input/output
schemas swapped and the decode/encode transforms swapped:

```ts
const stringToDate = z.codec(z.iso.datetime(), z.date(), {
  decode: (isoString) => new Date(isoString),
  encode: (date) => date.toISOString(),
});

const dateToString = z.invertCodec(stringToDate);
// z.decode(dateToString, new Date(...)) -> ISO string
// z.encode(dateToString, "2024-01-01T00:00:00Z") -> Date
```

Requirements:

- `invertCodec(codec)` returns a new codec; the original codec is not mutated
  and keeps working in its original direction.
- Decoding with the inverted codec validates against the original codec's
  OUTPUT schema and applies the original encode transform; encoding is the
  mirror image.
- Inverting twice round-trips to the original behavior.
- Provide it in BOTH the classic API (`zod`) and the mini API (`zod/mini`),
  following each API's existing conventions for codec helpers.
- Existing codec behavior (`z.codec`, `z.decode`, `z.encode`, safe variants)
  must be unchanged.
