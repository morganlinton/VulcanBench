# Replace the retired PayrollCore engine; remittance reconciliation rejects the rewrite's batches

We are decommissioning the PayrollCore withholding engine (a compiled
artifact from the 2014 acquisition; source long gone) and replacing it
with `payrollcore.py`. The rewrite follows `docs/SPEC.md` and matches on
simple batches, but ledger and remittance reconciliation fail on real
payroll files: net pay off by a cent on scattered records, certain
household filers withheld more than the rewrite computes, the social levy
diverging wildly on exactly one record per high earner and never again,
one state's employees withheld differently than documented, a flag
combination that visibly changes take-home pay although flags are
documented as informational, an undocumented pay period the engine
accepts, zero-gross no-op records handled completely differently, and
employees whose id was keyed twice with different capitalization coming
out merged.

The spec's own header says it: the document has drifted, and **the
engine's behavior is the contract**, because everything downstream was
built against the engine. The retired binary is in `legacy/` for
reference while you work (`legacy/run < batch.txt`, one batch per
process, year-to-date accumulates within a batch). It is NOT present in
production, so the replacement must reproduce its behavior, not invoke
it.

Make `payrollcore.py` a drop-in behavioral replacement: byte-for-byte
identical result lines and trailer for any batch, matching how the
engine actually validates, brackets, rounds, accumulates, caps, and
counts, wherever that differs from the spec. Where the spec IS accurate,
nothing may change.
