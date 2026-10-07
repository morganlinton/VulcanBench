# Frontier v5 Phase 1: toolchain images and the Harbor runner

Working log for Phase 1 of [PLAN.md](PLAN.md) section 7. Not frozen; this file
is updated as the pilots proceed. Started 2026-10-06.

## What exists after this step

| Item | State | Where |
| --- | --- | --- |
| Harbor CLI | 0.24.0 installed with `uv tool install harbor` (executables `harbor`, `hb`, `hr`) | host |
| C and C++ image | `vulcanbench/sandbox:cfamily`, built and smoke-tested | [sandbox/Dockerfile.cfamily](../../sandbox/Dockerfile.cfamily) |
| JVM image | `vulcanbench/sandbox:jvm`, built and smoke-tested | [sandbox/Dockerfile.jvm](../../sandbox/Dockerfile.jvm) |
| Native arm64 base | `vulcanbench/sandbox:base-arm64`, built from the arm64 digest of the same tag | [sandbox/Dockerfile.base](../../sandbox/Dockerfile.base) with `PYTHON_IMAGE` |
| Runner smoke test | passes: oracle reward 1, no-op reward 0, verifier network blocked | [scripts/harbor-smoke/run.sh](../../scripts/harbor-smoke/run.sh) |

Versions pinned by the images, to be copied into each task's determinism
checklist entry:

| Tool | Version |
| --- | --- |
| gcc, g++ | Debian 12.2.0-14+deb12u1 |
| clang, clang++, clang-tidy, lld, llvm-symbolizer | Debian LLVM 15.0.6 |
| cppcheck | 2.10 |
| cmake | 3.25.1 |
| meson | 1.0.1 |
| ninja | 1.11.1 |
| gdb | 13.1 |
| JDK | Eclipse Temurin 21.0.12.1+1 (sha256 per architecture in the Dockerfile) |
| Maven | 3.9.16 (sha512 in the Dockerfile) |
| Gradle | 9.8.0 (sha256 in the Dockerfile), daemon and parallel execution off |

Build commands used on this Mac (Apple silicon):

```bash
docker build --platform linux/arm64 --build-arg PYTHON_IMAGE=python:3.12-slim-bookworm@sha256:739ba32ae445e8d58f3d90feb85f83bebc8346f8dd280fa1eb5848f4ff1ed163 -t vulcanbench/sandbox:base-arm64 -f sandbox/Dockerfile.base .
```

```bash
docker build --platform linux/arm64 --build-arg BASE_IMAGE=vulcanbench/sandbox:base-arm64 -t vulcanbench/sandbox:cfamily -f sandbox/Dockerfile.cfamily .
```

```bash
docker build --platform linux/arm64 --build-arg BASE_IMAGE=vulcanbench/sandbox:base-arm64 -t vulcanbench/sandbox:jvm -f sandbox/Dockerfile.jvm .
```

On an amd64 host both arguments are omitted and the images build on the
existing `vulcanbench/sandbox:base`.

## Findings

Each of these changes something in how v5 tasks are authored or run.

1. **The base digest is amd64-only.** `Dockerfile.base` pins
   `python:3.12-slim-bookworm` by an image manifest digest, not an index.
   `docker build --platform linux/arm64` against it warns
   `InvalidBaseImagePlatform` and produces amd64 content labeled arm64. The
   first "arm64" base built this way was discarded. `Dockerfile.base` now
   takes `PYTHON_IMAGE` (default unchanged) and the header records the arm64
   digest resolved on 2026-10-06. The amd64 tag that other suites run on was
   not touched.

2. **ThreadSanitizer on linux/arm64 needs a seccomp allowance.** TSan
   re-executes itself with ASLR disabled through
   `personality(ADDR_NO_RANDOMIZE)`. Docker's default seccomp profile denies
   that call, so every TSan binary dies with
   `CHECK failed: tsan_platform_linux.cpp ... personality` inside a default
   container, including `docker build` RUN steps. The same binary reports the
   race under `docker run --security-opt seccomp=unconfined`. ASan, UBSan and
   MSan work under the default profile on arm64. Consequences: the image's
   build-time smoke test accepts the blocked outcome and the real TSan check
   runs after the build with the relaxed profile; any F1 task that uses TSan
   must declare the allowance and the runner must grant it (for Harbor's
   Docker environment the candidate is an `--extra-docker-compose` overlay
   with `security_opt`; unverified, pilot item). amd64 TSan does not need
   it, so a TSan task that passes on CI and segfaults on an arm64 host is this
   finding, not a flaky task. Add it to the C and C++ determinism checklists.

3. **Debian bookworm package naming.** The clang 15 sanitizer runtimes live in
   `libclang-common-15-dev`, a dependency of `clang-15`; `libclang-rt-N-dev`
   exists only from LLVM 16. The first build failed on that name.

4. **Harbor skips the tests upload when the verifier image is prebuilt.**
   `resolve_verifier_environment_definition` marks tests as bundled when
   `[verifier.environment].docker_image` is set or `tests/Dockerfile` (or
   `docker-compose.yaml`) exists, and the verifier then does not upload
   `tests/` at all. With a bare `docker_image` the verifier ran
   `bash /tests/test.sh` against an empty container and raised
   `RewardFileNotFoundError`. The contract for v5 is therefore
   `tests/Dockerfile` with the build context `tests/` and an explicit
   `COPY . /tests`, as the smoke task does. Only the fallback case (no
   verifier image, no tests Dockerfile) uploads `tests/` into a copy of the
   agent environment image. PLAN.md section 4 already names
   `tests/Dockerfile`; this is why it is required rather than optional.

5. **`harbor run -o` must be absolute.** A relative jobs directory is resolved
   against the compose project directory (`environment/` or `tests/`), and
   `docker compose cp` fails with `invalid output path`; the tar fallback
   covered artifact download but not the upload. Both the smoke runner and
   the future ingest script pass absolute paths.

6. **Effort passes through natively.** `harbor run --effort <level>` exists,
   and both `claude-code` and `codex` agents expose `reasoning_effort` with
   the levels low, medium, high, xhigh and max (codex also none and minimal).
   These map one to one onto VulcanBench's effort levels; there is no ultra,
   so the `[effort].blocked` rule holds by construction, and the harness
   still checks it before launching. Codex's `web_search` and Claude Code's
   `disable_web_search` are both settable, which the pilots will turn off.

7. **Verified by the smoke test, with the oracle and no-op agents and no model
   call:** a real `harbor run` end to end on Docker; separate verifier built
   from `tests/Dockerfile`; the top-level `artifacts` handoff (the file landed
   at its source path, `/app` itself did not); `reward.json` as a metric map
   with the headline `reward` plus diagnostics, parsed by Harbor into the job
   summary; `no-network` enforced in the verifier phase (an HTTPS request from
   `test.sh` fails). Harbor also started an egress-control sidecar image on
   its own for the network policy.

## Pilot 1: F1-c, openapv-malformed-bitstream-hardening (started 2026-10-06)

Task directory: `tasks/frontier-v5/openapv-malformed-bitstream-hardening/`
(pre-registration in its DESIGN.md). Source: AcademySoftwareFoundation/openapv
at cdb30f234da30a82ff8ccaf4f81658bdefd7e994, the last commit before a
late-September hardening wave; gold is nine upstream hardening commits that
cherry-pick cleanly onto it. Base and gold both pass the project's 24 ctest
tests under ASan and UBSan in release mode.

Lessons from building the hidden corpus, each one a line for the C
determinism checklist or the authoring guide:

1. **Oracle builds must be release builds.** The project's `oapv_assert`
   macros become libc `assert()` in Debug, so every malformed-input check
   aborts before any sanitizer can report. The oracle uses RelWithDebInfo
   (NDEBUG) plus sanitizers, which is also how the library ships.
2. **The fuzzed library needs its own coverage instrumentation.** Linking a
   `-fsanitize=fuzzer` harness against a library built with only
   `-fsanitize=address,undefined` gives libFuzzer about 50 edges, all in the
   harness; the first ten-minute run found nothing. The library is built
   with `-fsanitize=fuzzer-no-link` for corpus discovery (never for grading).
3. **Seeds must fit under the input cap.** The conformance access units are
   about 3 MB each (4K frames); with a 64 KB cap libFuzzer truncated every
   seed to a header. Small seeds are encoded from the project's own 320x240
   sequence with its encoder (7 KB to 17 KB per access unit, with tile,
   profile, quantization-matrix and frame-hash variants).
4. **The verifier probe shares the discovery harness.** Crashes found through
   the library API do not necessarily reproduce through the CLI app, which
   validates some fields itself, so `tests/probe_dec.c` is the harness with a
   file-driven `main`, compiled against the agent's library at grading time.
5. **Pinned upstream snapshots instead of vendored trees.** The repository
   at the base commit is 180 MB, mostly conformance bitstreams. Both the
   agent environment and the verifier image clone upstream at build time
   (public baseline) and verify the commit hash; the agent workspace is
   re-initialised as a single-commit repository so no later history leaks,
   and the agent-phase allowlist blocks GitHub.

6. **Verify sanitizer detectability on the base before authoring, on the
   grading architecture.** Forty minutes of instrumented fuzzing found no
   sanitizer report on the OpenAPV base, and reading each fixed site showed
   why: the pre-wave base already guards the shallow paths (exp-Golomb
   accumulates in u32 with a shift mask and a bound; the metadata parser
   checks end-of-buffer at every step), the payload-count limit bounds an
   unbounded list rather than a fixed array, and the dequant shift overflow
   lives in the generic and AVX paths while arm64 dispatches to NEON, whose
   wrap is a defined intrinsic UBSan cannot see. A PR title that says
   "fix heap buffer overflow" is not evidence that ASan fires on the base.
   Rule for F1 sourcing: before any task files are written, build the base
   with the oracle flags on the architecture that will grade and show at
   least three distinct sanitizer reports from inputs you can regenerate.
   OpenAPV is withdrawn from F1-c and kept as an F2 differential-parity
   candidate, where its size and clean sanitizer builds are assets.

7. **Fuzzing the base to discover the corpus does not work; source F1 from
   reproducer-bearing regressions instead.** The same wall hit twice more on
   libmbus (rscada/libmbus, PR #240): five minutes of combined ASan+UBSan
   fuzzing found 178 crashes, but every one was a single co-resident shift
   overflow in storage-number decoding that the gold does NOT fix (still live
   upstream), so it cannot be the oracle and the verifier excludes the shift
   check. The heap overflows the PR actually fixes (mishandled snprintf
   return values in XML output) did not reproduce under ASan-only fuzzing at
   5 or 15 minutes with inputs up to 64KB, because a single length-bounded
   M-Bus frame cannot make the XML exceed the output buffer by blind mutation.
   Across OpenAPV and libmbus, coverage-guided fuzzing from valid seeds did
   not once produce a base-crashing, gold-clean input for the specific fixed
   defect. Conclusion for F1 sourcing: do not pick a security-fix PR and then
   try to reach its defect by fuzzing. Pick a defect that already ships a
   reproducer input (an OSS-Fuzz regression, a CVE proof-of-concept, or a
   fuzz corpus entry committed with the fix). The reproducer is the corpus
   seed: confirm base-crashes and gold-clean on it, minimize, cluster, done.
   This also removes the fuzzer-luck variance from the x3 determinism gate.
   libmbus is withdrawn from F1-c for the same reason as OpenAPV; its scaffold
   at tasks/frontier-v5/libmbus-frame-xml-hardening is left in place but
   marked blocked-no-corpus.

Open at the time of writing: the fuzzing attribution that fills the family
table, the correctness gate (base 0, gold 1, x3 under Harbor), the
reverted-commit controls, and the first reference-model runs, which need
decision (b)'s credentials configured for Harbor's agents.

## Still unverified (pilot exit criteria)

- A real agent run (`-a claude-code` and `-a codex`) through Harbor:
  instruction delivery, `--effort` taking effect, `allowlist` during
  `agent.run()` with only the LLM API hosts, agent install under the public
  baseline, trajectory and token accounting in `result.json`.
- Resource enforcement: `cpus` and `memory_mb` from `task.toml` under the
  `--cpus limit` and `--memory limit` policies.
- Granting the TSan seccomp allowance through Harbor (finding 2).
- The harness side: `task.toml` loading, `harbor_ingest.py`, the strip step.
- Decisions (b) and (c) must be recorded in `docs/DECISIONS.md` before the
  first gated pilot run (PLAN.md section 8).


## Authoring run 2026-10-06 night: Python pipeline proven, F3-floor tension

- The Harbor-native Python task pipeline is validated end to end by
  tasks/frontier-v5/nx-group-betweenness-epic: env image (networkx cloned at a
  pinned base, history stripped, deps installed), the declared artifact
  /app/networkx extracted, a separate verifier that imports the agent sources
  in-tree on a private copy, overlays a hidden gold test file, and protects the
  guard wall by taking test files from the pristine image. Direct-Docker check:
  base workspace scores reward 0 (11 held-out tests fail, 29-test guard wall
  passes), gold workspace scores reward 1. Deterministic x3.
- Two Python-specific traps, both now designed around and worth the py-v1
  checklist: an editable `pip install -e .` silently shadows worktree edits, so
  the verifier imports strictly in-tree via PYTHONPATH; and read-write mounts
  let a swap contaminate the base worktree across runs, so validation mounts
  read-only and mutates only a container-local copy.
- Family-fit finding: that task is a concentrated multi-bug correctness task
  (2 files, 11 tests, 1 module), well below the frozen F3 volume floor (12
  files, 1000 lines, 25 tests, 3 modules). It is v4-shaped, not a v5 family.
  The hardest real-defect tasks (group betweenness, node cuts) are concentrated
  by nature, while F3 rewards breadth, so composing genuine difficulty into F3
  from real bug fixes is in tension. Open question for the owner: a fifth
  family for concentrated multi-bug correctness, a revived mid-band, or only
  admit F3 arcs large enough to meet the floor. The task is kept as a validated
  pipeline artifact and labeled UNSLOTTED, not forced into F3.
