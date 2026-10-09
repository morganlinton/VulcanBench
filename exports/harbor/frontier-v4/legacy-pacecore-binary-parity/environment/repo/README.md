# pacecore

Python replacement for the retired PaceCore ad-spend pacing engine.

- `pacecore.py`: the replacement module and CLI
  (`python pacecore.py < commands.txt`).
- `docs/SPEC.md`: the written pacing protocol spec. **Read its drift
  warning**: the spec is stale in places, and the ad servers and billing
  reconcilers that talk to PaceCore were built against the engine's
  actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
