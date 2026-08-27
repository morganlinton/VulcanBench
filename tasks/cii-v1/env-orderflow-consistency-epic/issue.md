# Flash-sale postmortem: double charges, phantom stock, orders frozen mid-flight, and a "repair" job that makes it worse

This is the orderflow platform (see `README.md`; `scripts/dev_up.py` boots
it against this run's live Postgres and Redis, published in
`.vb_services.json`). During Friday's flash sale, support and finance
escalated four incident threads. All of them reproduce on this snapshot.

1. **Customers were charged twice.** Finance found orders with two settled
   charges for different amounts (one with the promo discount, one
   without). The affected customers had double-clicked checkout, or our
   storefront had re-submitted after a slow response, while a promo code
   was being applied to their order. Billing's idempotency was supposed to
   make duplicate submissions safe.

2. **We oversold the flash SKUs.** Items with a hard stock of N granted
   far more than N holds under storefront load, and the on-site
   availability number stayed frozen at its pre-sale value all afternoon,
   even after the periodic bulk refresh ran. Separately, merch reports
   that after returns come back (holds expiring or released), the site
   understates what is actually sellable and turns away real sales.

3. **Orders froze at PAID.** Payments settled, stock was shipped, but the
   orders never advanced to COMPLETE no matter how many times the
   settlement processor ran. Meanwhile, warehouse noticed reservation rows
   that were somehow released back to the shelf AND shipped, and the stock
   counters have drifted into numbers that are plainly impossible.

4. **The nightly reconciliation made everything worse.** After the
   reconciliation job ran, orders that were verifiably stuck mid-pipeline
   showed up as COMPLETE in the ledger itself, and finance's morning
   report no longer matched what actually happened. Ops has disabled the
   job until someone can explain which direction it is supposed to fix.

Make the platform hold its invariants under exactly this kind of load:
a duplicated checkout never produces a second charge regardless of what
the price is doing at that moment; stock is never oversold and never
drifts, whatever mix of holds, ships, releases, and expiries runs
concurrently; availability reflects reality after returns and after the
bulk refresh; paid-and-shipped orders always reach COMPLETE; and
reconciliation repairs the derived view from the ledger without ever
rewriting the ledger.

The public API surface and all sequential behavior must not change:
pricing and coupon math, idempotent charge replay, checkout replay on an
already-paid order, availability served from the cache when warm,
expiry of genuinely abandoned holds, every documented error contract
(insufficient stock, unknown ids, validation), the cancel flow, and
exactly-once settlement processing.
