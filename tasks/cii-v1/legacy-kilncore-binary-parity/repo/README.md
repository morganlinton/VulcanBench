# kilncore

Python replacement for the retired KilnCore firing-lot controller.

- `kilncore.py`: the replacement module and CLI
  (`python kilncore.py < commands.txt`).
- `docs/SPEC.md`: the written firing protocol spec. **Read its drift
  warning**: the spec is stale in places, and the kiln schedulers and
  certification panels that talk to KilnCore were built against the
  engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
