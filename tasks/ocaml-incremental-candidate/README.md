# OCaml Incremental development candidate

One full-repository candidate, separate from both measured frozen pools. It is
validated and calibrated: GPT-6.1 Sol medium passed 3/3 fresh native attempts.
Read CALIBRATION.md for results and CHECKPOINT.md for receipts. It exercises
budgeted propagation in Jane Street's actual Incremental source, using the
existing OCaml 5.2.1 native toolchain and installed v0.17 dependencies.
The portable preflight compiles the library and upstream test library and runs
two public clients. Upstream debug/inline tests do not all execute; see
UPSTREAM_ALL_PROBE.json.

The offline and harness gates passed. Run on this Mac without Docker:

```sh
source /Users/vulcanbench/.local/share/vulcanbench/ocaml-v1-native/env.sh
export PATH=/Users/vulcanbench/.local/share/vulcanbench/ocaml-v1-native/codex-cli/node_modules/.bin:"$PATH"
vulcanbench run --tasks-root tasks/ocaml-incremental-candidate \
  --task incremental-budgeted-propagation --harness codex \
  --model gpt-6.1-sol --effort medium --billing subscription \
  --repeat 3 --max-concurrency 1 --sandbox local \
  --no-agent-container --no-judges --output-dir runs-ocaml-incremental-new
python scripts/report_ocaml_calibration.py \
  --suite ocaml-incremental-candidate --sandbox local \
  --runs-root runs-ocaml-incremental-new \
  --output runs-ocaml-incremental-new/MODEL_CARD.json \
  --card runs-ocaml-incremental-new/MODEL_CARD.html
```

The recorded calibration uses a serial single-attempt launcher that stops on
invocation failures rather than automatically retrying. Preserve new receipts
for every subsequent model. Do not shorten the 10800-second / 540-step budgets
or publish an under-80% full-suite claim from one candidate. Native model
execution has no Docker resource caps or network isolation. Reference gates
enforce offline execution. Model cards measure functional completion only.
