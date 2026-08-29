# Replace the retired DiffCore engine; restored snapshots are coming back wrong

We are decommissioning the legacy DiffCore snapshot-store engine (a
vendor binary whose source was lost when the vendor folded in 2019) and
replacing it with `diffcore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the backup
coordinators that drive DiffCore command streams, all built against the
engine's actual behavior over many years, are misbehaving batch after
batch:

- Restores are coming back with the WRONG digests: `T` replies that the
  engine produced for years now carry different `V` values through the
  new store, but only for some snapshots in a batch; restoring other
  snapshots from the very same stream matches perfectly.
- The divergence is history-dependent: which snapshots read back wrong
  seems to depend on how many snapshots were taken before them, on
  whether the working value shrank and grew again between stores, and on
  whether a restore happened earlier in the stream; snapshots taken
  right after a restore are especially unreliable.
- Reply streams are getting out of step: for some batches the new
  store's log has MORE lines than the engine's log of the same stream,
  every reply after a certain point is shifted by a line, and the `X`
  trailers no longer reconcile with the coordinators' own counts.
- The values themselves are never in the logs (only digests and counts),
  so nobody has been able to point at which stored bytes differ; the
  coordinators only see the digests disagreeing.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `diffcore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including digests, counts, error
codes, presence or absence of each reply line, and the trailer) for any
command stream, across sets, snapshot stores, and restores, matching how
the engine actually parses, validates, snapshots, reconstructs, and
counts, wherever that differs from what the spec says. Where the spec IS
accurate, nothing may change.
