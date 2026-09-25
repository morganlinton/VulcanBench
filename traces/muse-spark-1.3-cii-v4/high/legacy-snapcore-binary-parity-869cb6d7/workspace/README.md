# snapcore

Python replacement for the retired SnapCore session-state engine.

- `snapcore.py`: the replacement module and CLI
  (`python snapcore.py < commands.txt`).
- `docs/SPEC.md`: the written blob format spec. **Read its drift
  warning**: the spec is stale in places, and the session managers that
  export and import SnapCore blobs were built against the engine's actual
  behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
