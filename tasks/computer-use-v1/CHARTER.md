# VulcanBench ComputerUse v1

The charter for this suite is [docs/COMPUTER_USE_CHARTER.md](../../docs/COMPUTER_USE_CHARTER.md).

- Tasks are Harbor-native and run through Harbor's built-in `computer-1` agent.
- Admission checks: `scripts/computer-use/validate_task.py tasks/computer-use-v1/<task>`.
- Reference-model gate runs: `scripts/computer-use/run_gate.sh claude <task>`.
- Host setup (harbor extra and the `vulcanbench/sandbox:cu-base` image): `scripts/computer-use/setup-host.sh`.
