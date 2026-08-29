# tallycore

Python replacement for the retired TallyCore ranked-ballot tally engine.

- `tallycore.py`: the replacement module and CLI
  (`python tallycore.py < commands.txt`).
- `docs/SPEC.md`: the written protocol spec. **Read its drift warning**:
  the spec is stale in places, and the election-night reporting stack
  that consumes TallyCore streams was built against the engine's actual
  behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
