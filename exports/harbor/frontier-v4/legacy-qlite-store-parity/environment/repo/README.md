# qlite

Python replacement for the retired QLite record store.

- `qlite.py`: the replacement store and CLI (`python qlite.py < commands.txt`).
- `docs/SPEC.md`: the written protocol spec. **Read its drift warning**:
  the spec is stale in places, and the downstream reporting pipelines were
  built against the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < commands.txt`). It is not present in production.
