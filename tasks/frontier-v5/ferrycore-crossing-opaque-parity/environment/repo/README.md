# ferrycore

Python replacement for the retired FerryCore crossing-log engine.

- `ferry/ferrycore.py`: the replacement engine and CLI
  (`cd ferry && python3 ferrycore.py < session.txt`). Production deploys
  the `ferry/` directory only.
- `docs/SPEC.md`: the written command spec. **Read its drift warning**:
  the spec is stale in places, and the slip boards and reconciliation
  tooling were built against the engine's actual behavior, which is the
  contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < session.txt`). It is not present in production.
