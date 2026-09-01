# paddockcore

Python replacement for the retired PaddockCore pony field co-op
engine.

- `paddockcore.py`: the replacement engine and CLI
  (`python paddockcore.py < session.txt`).
- `docs/SPEC.md`: the written command spec. **Read its drift warning**:
  the spec is stale in places, and the co-op's rota boards and
  reconciliation tooling were built against the engine's actual
  behavior, which is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < session.txt`). It is not present in production.
