# OCaml candidate expansion

Three larger candidates for the Jane Street-relevant OCaml development pool.
They are evaluated separately from the frozen eight-task `ocaml-v1` suite.
The existing native library result, 19/21 complete passes (90.5%), is unchanged.
Fresh native GPT-6.1 Sol medium calibration completed all nine attempts:
7/9 complete passes (77.8%) for these three candidates. See
[calibration and its limits](CALIBRATION.md). The contextual ten-library total
is still 26/30 (86.7%); the full eleven-task result is withheld pending the
original compiler. The new trio's result is not a below-80% full-suite claim.

| Candidate | Scope | Primary view |
|---|---|---|
| [Durable delivery](async-durable-delivery/issue.md) | Async worker leases, keyed retries, ordered durable acknowledgements, cancellation and shutdown across 12 modules | Engineering |
| [Typed schema evolution](typed-schema-evolution/issue.md) | Heterogeneous GADT records, nested migration, exact legacy bytes, bounded transactional decoding | Language mastery |
| [Nested Base transactions](base-nested-transactions/issue.md) | Original feature extension to the full 495-file Base v0.17.3 checkout, including existing AVL mutation paths | Both |

These are original feature requests. None is presented as a known upstream bug,
Jane Street internal code, or a task endorsed by Jane Street. The Base source is
public and retrievable. See [research and provenance](RESEARCH.md), the
[requirement-to-check mapping](CHECKS.md), and Base's
[pinned provenance](base-nested-transactions/PROVENANCE.json).

The tasks contain 21 behavior groups and six regression guards. All supplied
interfaces are guarded, including Base's signature-defining hashtbl_intf.ml.
Hidden tests and references are injected only for grading. Public examples and
all task requirements are available during solving. Allocation ceilings and
clock semantics are explicit in the prompts; real-time sleeps are unnecessary.

## Native validation

Use the existing dedicated native OCaml 5.2.1 toolchain on this Mac. Its 96
package pins are preserved in [TOOLCHAIN.lock](TOOLCHAIN.lock). Dune 3.17.2
uses the release profile for all three tasks. The public Base development
profile raises upstream warning 67 under this compiler; the release profile
matches the package build convention. This is documented build compatibility,
not a behavior failure. No Docker is needed.

```sh
source /Users/vulcanbench/.local/share/vulcanbench/ocaml-v1-native/env.sh
PYTHONPATH="$PWD" .venv/bin/python scripts/validate_ocaml_pilot.py \
  --tasks-root tasks/ocaml-candidates --sandbox local --offline \
  --output /tmp/ocaml-candidates-validation-new.json
```

The gate requires three clean base/reference pairs per task, healthy baseline
regression guards, every behavior group failing at baseline, complete reference
passes, all eight compiling faulty controls rejected, and standard validation.
Network denial covers the entire validation process and its children.
Preserve receipts instead of overwriting old runs. The initial gate's metadata
failure is retained in VALIDATION_OFFLINE_INITIAL.json/log. Base's actual
66,082 code lines require the xlarge label. Subsequent serialization checks
also cover bounds and independent legacy fixtures. Neither correction reduces
the model solve allowance.

## Fresh measurement and cards

Use GPT-6.1 Sol medium, three fresh attempts per task, serial concurrency,
the existing 10800-second solve and 540 configured-step defaults, subscription
billing, no judges, and native local execution. The CLI step allowance is
recorded configuration, not a guaranteed enforced turn limit. Do not pool local
and Docker scores. Infrastructure errors and unscored provider refusals cannot
serve as model difficulty evidence. Do not change frozen task requirements
during calibration or select away failures.

For an individual native attempt:

```sh
source /Users/vulcanbench/.local/share/vulcanbench/ocaml-v1-native/env.sh
export PATH=/Users/vulcanbench/.local/share/vulcanbench/ocaml-v1-native/codex-cli/node_modules/.bin:"$PATH"
.venv/bin/vulcanbench run --task async-durable-delivery \
  --tasks-root tasks/ocaml-candidates --harness codex --billing subscription \
  --model gpt-6.1-sol --effort medium --repeat 1 --max-concurrency 1 \
  --sandbox local --no-agent-container --no-judges \
  --output-dir runs-ocaml-candidates-new
```

Create a separate functional card from candidate receipts:

```sh
.venv/bin/python scripts/report_ocaml_calibration.py \
  --suite ocaml-candidates --runs-root runs-ocaml-candidates-new \
  --sandbox local --output /tmp/ocaml-candidates-card-new.json \
  --card /tmp/ocaml-candidates-card-new.html
```

The headline remains withheld until every task has three current-hash scored
attempts in the selected environment. Task pass@1 requires all behavior and
regression checks to pass, then averages equally across tasks. The under-80%
goal is a measurement question, not an admission criterion or a reason to
weaken settings. Three tasks and nine development attempts are insufficient
for a stable leaderboard. Independent confirmation and additional task families
remain necessary. OCaml quality and security factors are not implemented.
The card is semantically tested; visual rendering remains unverified after
the app's local-file preview restriction. This expansion does not extend the
expired overnight automation.

See [the progress checkpoint](CHECKPOINT.md) for actual validation and calibration
state, limitations, and saved receipt locations.
