# vaultcore

Python replacement for the retired VaultCore versioned document vault.

- `vaultcore.py`: the replacement module and CLI
  (`python vaultcore.py < commands.txt`).
- `docs/SPEC.md`: the written vault spec. **Read its drift warning**: the
  spec is stale in places, and the archival pipelines that write, sweep,
  and audit the vault were built against the engine's actual behavior,
  which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
