# tariffcore

Python replacement for the retired TariffCore duty calculator.

- `tariffcore.py`: the replacement calculator and CLI
  (`python tariffcore.py < declarations.txt`).
- `docs/SPEC.md`: the written format spec. **Read its drift warning**: the
  spec is stale in places, and the customs reconciliation systems were
  built against the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < declarations.txt`). It is not present in production.
