# blendcore

Python replacement for the retired BlendCore ink-blending controller.

- `blendcore.py`: the replacement module and CLI
  (`python blendcore.py < commands.txt`).
- `docs/SPEC.md`: the written blending protocol spec. **Read its drift
  warning**: the spec is stale in places, and the shop-floor dispensers
  and batch reconcilers that talk to BlendCore were built against the
  engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
