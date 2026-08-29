# packcore

Python replacement for the retired PackCore bin-allocator engine.

- `packcore.py`: the replacement engine and CLI
  (`python packcore.py < session.txt`).
- `docs/SPEC.md`: the written command spec. **Read its drift warning**:
  the spec is stale in places, and the provisioners and audit tooling
  were built against the engine's actual behavior, which is the
  contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < session.txt`). It is not present in production.
