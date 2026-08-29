# foldcore

Python replacement for the retired FoldCore booklet imposition engine.

- `foldcore.py`: the replacement module and CLI
  (`python foldcore.py < commands.txt`).
- `docs/SPEC.md`: the written imposition spec. **Read its drift
  warning**: the spec is stale in places, and the prepress controllers
  that drive imposition and consume sheet layouts were built against the
  engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
