# codeccore

Python replacement for the retired CodecCore VX interchange codec.

- `codeccore.py`: the replacement module and CLI
  (`python codeccore.py < commands.txt`).
- `docs/SPEC.md`: the written format spec. **Read its drift warning**: the
  spec is stale in places, and the partner systems on both sides of the
  interchange were built against the engine's actual behavior, which is
  the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
