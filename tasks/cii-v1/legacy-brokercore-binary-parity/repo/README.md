# brokercore

Python replacement for the retired BrokerCore freight load-board
controller.

- `brokercore.py`: the replacement module and CLI
  (`python brokercore.py < commands.txt`).
- `docs/SPEC.md`: the written load-board protocol spec. **Read its drift
  warning**: the spec is stale in places, and the dispatcher terminals
  and settlement clerks' batch tools that talk to BrokerCore were built
  against the engine's actual behavior, which is the contract.
- `legacy/`: the retired engine binary, available for reference while
  developing (`legacy/run < commands.txt`). It is not present in
  production.
