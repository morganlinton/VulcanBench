# cellarcore

Python replacement for the retired CellarCore cellar-ledger engine.

- `cellarcore.py`: the replacement engine and CLI
  (`python cellarcore.py < session.txt`).
- `docs/SPEC.md`: the written command spec. **Read its drift warning**:
  the spec is stale in places, and the cellar clerks' terminals and the
  seasonal reconciliation tooling were built against the engine's
  actual behavior, which is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < session.txt`). It is not present in production.
