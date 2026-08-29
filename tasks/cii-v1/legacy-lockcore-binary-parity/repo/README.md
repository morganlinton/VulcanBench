# lockcore

Python replacement for the retired LockCore lease-manager engine.

- `lockcore.py`: the replacement engine and CLI
  (`python lockcore.py < session.txt`).
- `docs/SPEC.md`: the written command spec. **Read its drift warning**:
  the spec is stale in places, and the coordination services and
  operator tooling were built against the engine's actual behavior,
  which is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < session.txt`). It is not present in production.
