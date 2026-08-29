# stampcore

Python replacement for the retired StampCore mailroom postage-meter
controller.

- `stampcore.py`: the replacement module and CLI
  (`python stampcore.py < commands.txt`).
- `docs/SPEC.md`: the written metering protocol spec. **Read its drift
  warning**: the spec is stale in places, and the mailroom terminals and
  postage reconcilers that talk to StampCore were built against the
  controller's actual behavior, which is the contract.
- `legacy/`: the retired controller binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
