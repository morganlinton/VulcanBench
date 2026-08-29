# Replace the retired ReplayCore ledger; nightly replay reconciliation is drifting

We are decommissioning the legacy ReplayCore event-sourced balance
ledger (a vendor binary whose source was lost in the 2019 handover) and
replacing it with `replaycore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the downstream
reconciliation and settlement systems, which were all built against the
engine's actual behavior over many years, keep flagging mismatches:

- The nightly replay reconciliation (`Y`) no longer agrees with the old
  engine's numbers. Ops reports that after a batch containing declined
  debits, the engine's replayed balances came back LOWER than the live
  balances and everything downstream reconciled against those lower
  numbers; the rewrite reports live-equal balances and now the two sides
  disagree account after account.
- Replay listings from the engine sometimes include accounts nobody ever
  successfully credited, and list accounts in a different order than the
  rewrite does, so positional diff tooling on the `Y` output blows up.
- Upstream producers redeliver events after timeouts. The old engine
  acknowledged some of those duplicate deliveries as applied; the
  rewrite bounces them as out of order, and the producers now retry
  forever.
- Batch trailers (`X` counts) no longer reconcile with producer logs,
  and some event streams that the engine accepted are now rejected
  outright.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `replaycore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical output (event replies, rejection codes and
echoes, every `Y` replay listing including its account order, and the
trailer) for any command stream, matching how the engine actually
parses, validates, applies, logs, and replays events, wherever that
differs from what the spec says. Where the spec IS accurate, nothing may
change.
