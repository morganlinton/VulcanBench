# diffcore

Python replacement for the retired DiffCore snapshot-store engine.

- `diffcore.py`: the replacement module and CLI
  (`python diffcore.py < commands.txt`).
- `docs/SPEC.md`: the written command and snapshot format spec. **Read
  its drift warning**: the spec is stale in places, and the backup
  coordinators that drive DiffCore command streams were built against
  the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
