# LR table wire format

`vyre-grammar-gen` ships LR table packing, dimension validation, and
checksummed wire decode/encode. The CLI emits LR blobs from explicit
caller-supplied `LrTable` JSON; it does not manufacture a synthetic C11 table.

## Supported Contract

- **Action/goto model**: `lr::{Action, Production, LrTable, LrBuilder}`.
- **Validation**: `validate_lr_table` checks action and goto dimensions before
  serialization.
- **Wire**: `PackedBlob::from_lr` / `decode_lr_from_bytes` round-trip with a
  BLAKE3-128 payload tag.
- **CLI**: `emit --lr-json <path>` and `dump-lr --lr-json <path>` serialize a
  concrete table supplied by the caller.

## Ownership Boundary

The production C frontend in this repository uses the VAST/PG parser pipeline
under `vyre-libs::parsing::c::parse`. LR table serialization remains available
for consumers that already have a concrete LR grammar table to upload.
