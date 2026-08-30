# groovecore

Python replacement for the retired GrooveCore vinyl-pressing queue
controller.

- `groovecore.py`: the replacement module and CLI
  (`python groovecore.py < commands.txt`).
- `docs/SPEC.md`: the written pressing protocol spec. **Read its drift
  warning**: the spec is stale in places, and the lathe consoles and
  pressing-floor reconcilers that talk to GrooveCore were built against
  the controller's actual behavior, which is the contract.
- `legacy/`: the retired controller binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
