# ferrycore

Python replacement for the retired FerryCore island car-ferry boarding
controller.

- `ferrycore.py`: the replacement module and CLI
  (`python ferrycore.py < dayfile.txt`).
- `docs/SPEC.md`: the written boarding protocol spec. **Read its drift
  warning**: the spec is stale in places, and the slipway terminals and
  season reconcilers that talk to FerryCore were built against the
  controller's actual behavior, which is the contract.
- `legacy/`: the retired controller binary, available for reference while
  developing (`legacy/run < dayfile.txt`). It is not present in
  production.
