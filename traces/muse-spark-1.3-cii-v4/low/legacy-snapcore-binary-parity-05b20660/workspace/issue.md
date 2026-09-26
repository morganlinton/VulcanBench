# Replace the retired SnapCore engine; session restore is corrupting state

We are decommissioning the legacy SnapCore session-state engine (a vendor
binary whose source was lost when the vendor folded in 2019) and
replacing it with `snapcore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the session
managers that export and import SnapCore blobs, all built against the
engine's actual behavior over many years, are misbehaving batch after
batch:

- State restored from older exports comes back wrong: some archived blobs
  that the engine imported for years are now rejected outright (`R VER`,
  `R FMT` on blobs that used to come back `OK`), and others import but
  then read back differently key by key.
- Sessions restored through the new store come back REORDERED: an import
  followed by a re-export produces the keys in a different order than the
  engine did, but only for some imports, and the discrepancy sometimes
  only shows up in the export after the next few writes.
- Some exports are losing characters: a handful of long values read back
  fine live but come out short after an export/import cycle through the
  engine, and the new store does not reproduce that, so the two sides'
  blobs no longer match byte for byte.
- Counts are off after imports: the `OK <n>` acknowledgments the session
  managers log for some imports disagree with what the engine reported
  for the same blob, and batch trailers (`X` counts) no longer reconcile
  with their logs.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `snapcore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including blobs, checksums, counts,
error codes, and the trailer) for any command stream, across live puts
and gets, serialization, and loading, matching how the engine actually
parses, validates, stores, serializes, and counts, wherever that differs
from what the spec says. Where the spec IS accurate, nothing may change.
