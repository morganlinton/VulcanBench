# choircore

Python replacement for the retired ChoirCore village-choir seating
engine.

- `choircore.py`: the replacement engine and CLI
  (`python choircore.py < season.txt`).
- `docs/SPEC.md`: the written seating protocol spec. **Read its drift
  warning**: the spec is stale in places, and the rota boards and
  seating printers that talk to ChoirCore were built against the
  engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < season.txt`). It is not present in
  production.
