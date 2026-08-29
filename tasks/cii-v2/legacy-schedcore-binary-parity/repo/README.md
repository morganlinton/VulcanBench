# schedcore

Python replacement for the retired SchedCore dependency-scheduler
engine.

- `schedcore.py`: the replacement engine and CLI
  (`python schedcore.py < session.txt`).
- `docs/SPEC.md`: the written command spec. **Read its drift warning**:
  the spec is stale in places, and the pipeline controllers and operator
  tooling were built against the engine's actual behavior, which is the
  contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < session.txt`). It is not present in production.
