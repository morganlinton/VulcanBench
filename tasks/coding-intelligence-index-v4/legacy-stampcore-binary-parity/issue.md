# Replace the retired StampCore controller; meters and audits no longer reconcile

We are decommissioning the legacy StampCore postage-meter controller (a
vendor binary whose source was lost when the vendor folded in 2019) and
replacing it with `stampcore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the mailroom
terminals and postage reconcilers, all built against the controller's
actual behavior over many years, are flagging discrepancies batch after
batch:

- Meters bleed balance the ledger cannot see: on busy meters the old
  controller's remaining-balance replies fall behind what the franked
  postage accounts for, a tenth at a time, and the ledger only catches
  up at the next zero-reading audit, which the old controller reports
  as `DRIFT` with the missing amount and then treats as settled without
  putting the money back. The replacement never bleeds and its audits
  always come back `MATCH`, so the reconcilers' tamper-investigation
  queue went quiet on meters that used to drift every week, and every
  remaining-balance figure runs ahead of what the old controller would
  have shown.
- Return credits pay at the wrong rate on busy meters: for meters with
  a lot of franking since their last audit, return credits the old
  controller granted disagree with pieces-times-last-postage, but only
  sometimes, and an audit right before the return makes the
  disagreement go away. The replacement always pays exactly the
  documented rate.
- Numbered meters overcharge when they run low: on meters whose id
  ends in a digit, once the balance gets low each frank costs a little
  more than the postage, in odd jumps; identically used meters with
  letter-ending ids charge exactly the postage. The replacement
  charges everyone exactly the postage.
- Small postage is refused some days: mailpieces at the small end of
  the postage range that always franked fine get `N ... POST` from the
  old controller on some meters on some days, then frank fine again
  after the meter is topped up. The replacement accepts them
  everywhere.
- Returns that always credited now reject: return batches the old
  controller quietly settled, sometimes for less than the pieces
  claimed, get `N ... RET` from the replacement; and a few meter
  registrations the old controller refused as duplicates now succeed,
  splitting one meter's franking across two ledgers. Feeds with extra
  trailing fields that always imported cleanly now throw `N ... FMT`.
  Batch trailers (`X` counts) no longer reconcile with the reject logs
  either way.

The spec's own header warns about this: the document has drifted, and
**the controller's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `stampcore.py` a drop-in behavioral replacement for the controller:
byte-for-byte identical reply lines (including remaining balances,
return credits, audit verdicts and deltas, error codes, and the trailer)
for any command stream, across meter registration, franking, postal
returns, and audits, matching how the controller actually parses,
validates, deducts, credits, audits, and counts, wherever that differs
from what the spec says. Where the spec IS accurate, nothing may change.
