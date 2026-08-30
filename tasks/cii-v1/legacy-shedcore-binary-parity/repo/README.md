# shedcore

Python replacement for the retired ShedCore tool-lending service-rota
controller.

- `shedcore.py`: the replacement module and CLI
  (`python shedcore.py < commands.txt`).
- `docs/SPEC.md`: the written lending protocol spec. **Read its drift
  warning**: the spec is stale in places, and the shed desk console and
  seasonal reconcilers that talk to ShedCore were built against the
  controller's actual behavior, which is the contract.
- `legacy/`: the retired controller binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
