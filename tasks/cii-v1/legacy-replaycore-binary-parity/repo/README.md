# replaycore

Python replacement for the retired ReplayCore event-sourced balance
ledger.

- `replaycore.py`: the replacement module and CLI
  (`python replaycore.py < commands.txt`).
- `docs/SPEC.md`: the written protocol spec. **Read its drift warning**:
  the spec is stale in places, and the downstream reconciliation and
  settlement systems were built against the engine's actual behavior,
  which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
