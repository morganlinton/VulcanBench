# granarycore

Python replacement for the retired GranaryCore grain co-op ledger
engine.

- `granarycore.py`: the replacement engine and CLI
  (`python granarycore.py < session.txt`).
- `docs/SPEC.md`: the written command spec. **Read its drift warning**:
  the spec is stale in places, and the co-op's reconciliation and
  audit tooling were built against the engine's actual behavior, which
  is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < session.txt`). It is not present in production.
