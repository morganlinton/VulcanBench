# Replace the retired ReflowCore engine; exported layouts no longer reconcile

We are decommissioning the legacy ReflowCore text-layout engine (a vendor
binary whose source was lost when the vendor folded in 2017) and
replacing it with `reflowcore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the editor front
ends and export pipelines that lay documents out through the engine, all
built against the engine's actual behavior over many years, are
misbehaving batch after batch:

- Documents grow a line after exports: a document typed in word by word
  reconciles with the editor's line counts, but the export step (which
  reflows the document) has always come back from the engine with an
  extra line here and there, and the new store instead reports the
  typed-in counts, so every export diff against the archived layouts now
  flags line-count regressions the pipelines have never seen.
- Digests drift after window resizes: front ends that change the wrap
  width mid-session log layout digests from the engine that the new
  store cannot reproduce, even when no reflow was requested, and the
  drift persists across all later commands in the session.
- Oversized tokens are accepted and then come back short: pipelines have
  always been able to push tokens wider than the current wrap setting
  straight through the engine, but layouts exported later show those
  tokens shortened, and the new store instead refuses the tokens
  outright (`E WORD`), halting replays that used to run clean.
- Digests disagree on freshly exported documents too: after any reflow,
  the engine's logged digests differ from the new store's for identical
  command streams, and the batch trailers (`X` counts) no longer
  reconcile with pipeline logs either.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `reflowcore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including line counts, digests,
error codes, and the trailer) for any command stream, across width
changes, appends, reflows, and digests, matching how the engine actually
parses, validates, lays out, and counts, wherever that differs from what
the spec says. Where the spec IS accurate, nothing may change.
