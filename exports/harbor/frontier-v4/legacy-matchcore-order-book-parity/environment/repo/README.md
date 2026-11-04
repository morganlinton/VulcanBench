# matchcore

Python replacement for the retired MatchCore matching engine.

- `matchcore.py`: the replacement engine and CLI
  (`python matchcore.py < session.txt`).
- `docs/SPEC.md`: the written protocol spec. **Read its drift warning**:
  the spec is stale in places, and every downstream consumer was certified
  against the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < session.txt`). It is not present in production.
