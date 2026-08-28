# tollcore

Python replacement for the retired TollCore passage rating engine.

- `tollcore.py`: the replacement module and CLI
  (`python tollcore.py < passages.txt`).
- `docs/SPEC.md`: the written format spec. **Read its drift warning**: the
  spec is stale in places, and every consumer downstream was built against
  the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < passages.txt`). It is not present in
  production.
