# hedgecore

Python replacement for the retired HedgeCore FX position ledger.

- `hedgecore.py`: the replacement ledger and CLI
  (`python hedgecore.py < session.txt`).
- `docs/SPEC.md`: the written ledger format spec. **Read its drift
  warning**: the spec is stale in places, and the marking and
  reconciliation systems that consume HedgeCore net values were built
  against the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < session.txt`). It is not present in
  production.
