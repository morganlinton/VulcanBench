# quotacore

Python replacement for the retired QuotaCore API-metering engine.

- `quotacore.py`: the replacement engine and CLI
  (`python quotacore.py < batch.txt`).
- `docs/SPEC.md`: the written format spec. **Read its drift warning**: the
  spec is stale in places, and the invoicing and dunning systems were
  built against the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, for reference while developing
  (`legacy/run < batch.txt`). It is not present in production.
