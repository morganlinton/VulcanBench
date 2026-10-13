# payrollcore

Python replacement for the retired PayrollCore withholding engine.

- `payrollcore.py`: the replacement engine and CLI
  (`python payrollcore.py < batch.txt`).
- `docs/SPEC.md`: the written format spec. **Read its drift warning**: the
  spec is stale in places, and the ledger and remittance systems were
  built against the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < batch.txt`). It is not present in production.
