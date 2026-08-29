# Replace the retired PaceCore engine; pacing and true-ups no longer reconcile

We are decommissioning the legacy PaceCore ad-spend pacing engine (a
vendor binary whose source was lost when the vendor folded in 2020) and
replacing it with `pacecore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the ad servers
and billing reconcilers, all built against the engine's actual behavior
over many years, are flagging discrepancies batch after batch:

- Daily true-up reports do not reconcile: for many campaigns the
  `U <total> <carry>` line the engine used to emit disagrees with the
  sum of the `P` grants the ad servers logged live for the same day,
  usually on days where spend landed late; the new engine's true-ups
  match the live grants exactly, so the two sides' reports no longer
  agree with each other, and the carried-over amounts drift apart from
  the first mismatched day onward.
- First-hour starvation is gone, and the reconcilers notice: after days
  with a lot of unspent budget, the old engine used to grant little or
  nothing in the first hour of the next day, sometimes for several days
  in a row; the replacement grants the full allowance and cumulative
  spend runs ahead of what the engine would have allowed.
- Large accounts pace differently overnight: for some high-budget
  campaigns, early-morning requests that the engine used to fill only
  partway are now filled in full, but only for some campaigns and only
  at some hours.
- Requests the engine used to reject now come back as tiny grants (and
  vice versa): a handful of same-hour follow-up requests that always
  logged an `N ... REQ` from the engine now log `P 0`.
- Feeds that always imported cleanly now throw rejects: some upstream
  command streams that the engine accepted for years get `N ... HOUR`
  or `N ... FMT` lines from the replacement, and a few campaign creates
  that the engine refused as duplicates now succeed, splitting one
  campaign's spend across two ledgers. Batch trailers (`X` counts) no
  longer reconcile with the reject logs either way.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `pacecore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including grants, true-up totals,
carries, counts, error codes, and the trailer) for any command stream,
across campaign creation, live spend requests, and end-of-day true-ups,
matching how the engine actually parses, validates, paces, audits, and
counts, wherever that differs from what the spec says. Where the spec IS
accurate, nothing may change.
