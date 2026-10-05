# VulcanBench OCaml v1

Eight validated development tasks balance OCaml engineering and language mastery.
Seven original projects cover typed transformations, collections, allocation,
Async, and Incremental. The eighth repairs an actual upstream compiler bug
involving GADTs and GC-root classification. Library tasks use pinned Jane Street
packages.

Docker-free execution is validated on this Darwin arm64 Mac. All eight tasks
passed three clean offline baseline/reference pairs, every guard, all 15
compiling faulty controls, and standard validation. The final CI passed 1092
tests. Native provisioning and functional JSON/HTML model-card commands are in
the measurement plan. A fresh native sweep completed 21 attempts across the
seven library tasks: 19/21 complete passes (90.5%). Six tasks passed 3/3, and
the incremental graph passed 1/3. See the separate native calibration record.

The earlier Docker cohort scored 17/21 complete passes (81.0%) on GPT-6.1 Sol at
medium effort. The native cohort also exceeds the requested under-80% target.
Keep the two execution environments separate. The current
eight-task score is withheld: the revised compiler task has zero fresh calibrated
outcomes. Its old core-only pass and an unscored provider refusal are preserved
separately. These are development results, not independent confirmation.

## Start here

- [Charter](CHARTER.md): task coverage, grading rules, and intended use.
- [Jane Street research](JANE_STREET_RESEARCH.md): public sources and the design rationale.
- [Validation](VALIDATION.md): 50 behavior checks, 20 guards, 15 rejected compiling controls, and a full cold compiler reference build.
- [Calibration](CALIBRATION.md): every scored outcome, revisions, and remaining difficulty gaps.
- [Native calibration](NATIVE_CALIBRATION.md): 19/21 library-task passes and the complete receipt audit.
- [Model patch review](MODEL_PATCH_REVIEW.md): clean source replay and engineering observations beyond the score.
- [Native environment](NATIVE_ENVIRONMENT.json): package pins, native compiler seed, and frozen suite identity.
- [Measurement plan](MEASUREMENT_PLAN.md): image builds, clean validation, and serial calibration commands.
- [Candidate record](CANDIDATES.md): per-task decisions and expansion priorities.
- [Overnight checkpoint](OVERNIGHT.md): current operational state and preserved work history.

## Build and validate

Native execution on this Mac is the owner's preferred path. Install the native
prerequisites (`opam`, `pkg-config`, and the Apple command-line tools), then run:

```sh
make ocaml-native
source "$HOME/.local/share/vulcanbench/ocaml-v1-native/env.sh"
python scripts/validate_ocaml_pilot.py --sandbox local --offline --output /tmp/ocaml-native-validation.json
```

Provisioning downloads dependencies once into a dedicated opam root and builds
a Darwin arm64 compiler seed from the public pre-fix source. All library package
versions must match TOOLCHAIN.lock. Clean native validation uses fresh workspaces
and denies network through macOS sandbox-exec. A Linux build seed is not usable
on this Mac. Model runs use `--sandbox local --no-agent-container`, retaining
the generous solve budgets and serial concurrency. Host runs have no Docker
resource ceilings and should be reported separately from the earlier Docker
calibration. See the measurement plan for the full run and model-card commands.

With the repository's Python 3.12 environment, Git LFS checkout, and Docker
ready, the Docker execution path uses these commands from the repository root:

```sh
make agent-image-ocaml-compiler
.venv/bin/python scripts/validate_ocaml_pilot.py --output /tmp/ocaml-validation.json
make ci
```

The image target builds both agent/verifier families and their prerequisites.
Clean validation is serial and repeats reference checks; compiler rebuilding can
take substantial time. No model call is made by these commands. Model calibration
uses the matching task image and the existing three-hour solve ceiling, three
fresh attempts, and serial concurrency. See the measurement plan before launching.

The original projects use OCaml 5.2.1 and pinned v0.17 libraries in
[TOOLCHAIN.lock](TOOLCHAIN.lock). The compiler-under-test is upstream 5.6.0+dev0;
[its provenance](compiler-gadt-field-safety/PROVENANCE.json) records source, reference,
images, and public baseline build seed. This is an upstream OCaml suite. OxCaml
extensions require a separate future track.

Public upstream patches may be retrievable. Source freshness does not establish
training cutoffs, and relevance to Jane Street is inferred from public material,
not internal review. Functional checks do not certify maintainability or security;
the current OCaml quality/security analyzers are unavailable.
