# reflowcore

Python replacement for the retired ReflowCore text-layout engine.

- `reflowcore.py`: the replacement module and CLI
  (`python reflowcore.py < commands.txt`).
- `docs/SPEC.md`: the written layout spec. **Read its drift warning**:
  the spec is stale in places, and the editor front ends and export
  pipelines that lay out documents through the engine were built against
  the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
