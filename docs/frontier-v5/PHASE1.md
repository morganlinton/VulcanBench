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

## Rust pipeline proven 2026-10-07

tasks/frontier-v5/petgraph-maxflow-sparse-index validates end to end OFFLINE
(base reward 0, gold reward 1, build_ok in both). It proves the Rust half of
the toolchain that the plan flagged as mandatory and unproven: a native arm64
Rust image (sandbox/Dockerfile.rust-arm64-min, a minimal stopgap; the admitted
image should be the full pinned Dockerfile.rust built for arm64), a generated
and committed Cargo.lock, dependencies fetched at environment-build time, and
both the agent and the verifier building and testing with `cargo --offline`
(net.offline baked into CARGO_HOME). The task itself is easy by design (one
root cause, panic points near the line) and is kept as a pipeline artifact and
easy Rust anchor, not a frontier-difficulty candidate.

## JavaScript pipeline proven 2026-10-07

tasks/frontier-v5/luxon-duration-format-fixedzone validates end to end (base
reward 0, gold reward 1, x3, plus single-fix controls) on the existing
`vulcanbench/sandbox:base-arm64` image, which already carries Node 22.11.0
and npm 10.9.0; no new image was needed. Lessons for the js-v1 checklist:

- Luxon's tests import `src/` directly through babel-jest, so the graded
  artifact is the agent's `src/` tree and the verifier runs the pristine
  suite in-tree against it; no build output of the agent is used. Test
  files, jest and babel configs and `node_modules` come from the verifier
  image.
- Dependencies come from the committed `package-lock.json` with
  `npm ci --ignore-scripts` at image build time (the public baseline);
  `--ignore-scripts` skips the husky `prepare` hook, which needs a git tree
  the stripped workspace no longer has.
- Upstream CI's environment matters: `TZ=America/New_York` and a UTF-8
  locale are set in both images, as in luxon's workflow.
- ICU is part of the toolchain pin. Node 22.11's ICU 75 names the Islamic era
  `ERA1` where upstream CI's newer ICU prints `AM`, so two locale tests fail
  at base and gold alike. The verifier runs the whole suite as the guard wall
  and excludes those two by name (`known_env_failures` in families.json),
  pins the total test count, and treats any other failure or any test file
  that fails to load as a broken wall. Record the ICU version with Node's.
- Grading reads jest's `--json` report and matches tests by
  `<file>::<fullName>`; single-fix controls (each PR applied alone) confirm
  each cause is independently exercised.

## Java pipeline proven 2026-10-07

tasks/frontier-v5/commons-lang-fraction-lowest-terms validates end to end
(base reward 0, gold reward 1, x3, five single-fix controls) on
`vulcanbench/sandbox:jvm` built on the arm64 base. Lessons for the java-v1
checklist:

- Offline Maven is a two-step image build: one ONLINE `mvn clean test`
  (with `-Dtest=<one class>`) populates a task-local repository
  (`/opt/m2`, about 90 MB for Commons Lang), and only then is
  `.mvn/maven.config` written with `-o -Dmaven.repo.local=/opt/m2` plus the
  analysis-plugin skips (rat, checkstyle, spotbugs, jacoco, pmd, animal
  sniffer, spotless, japicmp, cyclonedx, javadoc). Writing the config before
  the warm-up run makes the warm-up itself offline and the parent POM
  unresolvable. Every goal the agent or verifier will call must be in the
  warm-up: `mvn clean` failed offline until `clean` joined it.
- The workspace is committed as a single commit after the warm-up so the
  offline config is part of the snapshot; `target/` is cleaned first.
- The graded artifact is `src/main`; the verifier compiles it together with
  the pristine test sources (`test-compile` failure is reward 0) and runs
  `mvn test` with the pristine config, parsing surefire XML by
  `classname::name` (`tests/check.py`).
- Commons Lang's full suite (630 classes, 89,192 executions) is clean and
  deterministic in the image at `TZ=UTC`, 2.5 minutes single, 3 to 4
  minutes with three verifiers sharing the host, so the whole suite is the
  guard wall with the unique-id count pinned; no environment exclusions
  were needed (contrast luxon's two ICU-dependent tests).
- A control that does not apply alone on the base (a later fix's context
  depends on an earlier one) is tested as gold minus that fix instead.

## Portable image tags 2026-10-07

The validated tasks were built against host-local tags that named the
laptop's architecture (`vulcanbench/sandbox:base-arm64`, `:rust-arm64-min`)
or the amd64 default (`:jvm`, `:cfamily` built on `base-arm64`). To run the
same task Dockerfiles on an x86-64 cloud VM, every v5 task now starts from
an architecture-neutral tag, `vulcanbench/sandbox:v5-base`, `:v5-cfamily`,
`:v5-jvm` or `:v5-rust`, and `scripts/frontier-v5/setup-host.sh` builds
those four for whatever CPU it runs on (arm64 digest on Apple silicon, the
Dockerfile default on amd64). `Dockerfile.rust` gained the same `BASE_IMAGE`
argument `Dockerfile.jvm` and `Dockerfile.cfamily` already had, and
`:v5-rust` is the full pinned Rust image (clippy, rustfmt, cargo-audit), not
the minimal stopgap `Dockerfile.rust-arm64-min` the petgraph task was first
validated on; petgraph was re-validated on `:v5-rust`. Tool versions inside
the tags are unchanged; only the names are. The earlier validation records
that name `base-arm64` describe the same content.

## Cloud VM setup 2026-10-07 (first run on x86-64)

First run of `scripts/frontier-v5/setup-host.sh` on a Claude Code cloud VM
(x86-64, 4 vCPU, 16 GB RAM, Ubuntu 24.04 host, Docker 29.8.2). It did not
reach "frontier-v5 setup: done". What broke, in the order it surfaced:

1. **The Docker daemon is not running at session start.** `dockerd` and
   `containerd` are installed but nothing launches them; the script stopped
   at "docker daemon not reachable". Started by hand with `dockerd` in the
   background (overlayfs, cgroup v1). The environment's setup script should
   start it before calling setup-host.sh.
2. **Build containers sit behind a TLS-intercepting egress gateway.** A
   container on the default bridge network gets every HTTPS connection
   re-signed by "Egress Gateway SDS Issuing CA", and plain-HTTP requests
   (apt's default `http://deb.debian.org`) are refused with 403. The pinned
   python image does not trust that CA, so every fetch in the chain fails
   certificate verification. Note the host's agent-proxy CA
   (`/root/.ccr/agent-proxy-ca.crt`) is not the one that signs container
   traffic; the gateway CAs are in the host's
   `/usr/local/share/ca-certificates/`. Fixed in the script: on a host
   marked by `/root/.ccr` (or with `--ca DIR_OR_FILE`) it first builds a
   local trust layer, `vulcanbench/sandbox:v5-python-ca`, from the same
   pinned digest with those CAs in the system store, Debian sources switched
   to HTTPS (the same signed packages), and `SSL_CERT_FILE`,
   `REQUESTS_CA_BUNDLE`, `PIP_CERT` and `NODE_EXTRA_CA_CERTS` set, then
   builds v5-base on it. `Dockerfile.jvm` imports the same CAs into
   Temurin's own cacerts when the layer is present (a no-op otherwise).
   Verified: inside the layer pypi.org, nodejs.org and github.com verify and
   return 200. Without CAs (laptop, CI) the script and images are unchanged.
   The CAs and env vars ride along into every cloud-built task image; they
   change nothing at grading (the verifier runs with no network) but they
   are a recorded cloud-only difference from laptop-built images.
3. **The environment's network policy denies the Debian archive.** With TLS
   fixed, apt gets `403 Forbidden` from the gateway on
   `https://deb.debian.org` (the main, updates and security suites are all
   served from that host). This is an organization egress-policy denial,
   not a script bug, and it is not routed around. The same policy denies
   `go.dev` and `dl.google.com` (the Go download in `Dockerfile.base`) and
   `dlcdn.apache.org` (Maven in `Dockerfile.jvm`). Reachable: Docker Hub,
   github.com, nodejs.org, pypi.org, index.crates.io, static.rust-lang.org,
   services.gradle.org, repo.maven.apache.org. Action for the owner: add
   `deb.debian.org`, `go.dev`, `dl.google.com` and `dlcdn.apache.org` to the
   cloud environment's allowed domains (Network access, with the package
   managers box ticked), then re-run setup-host.sh. Until then no v5 image
   builds on the cloud VM.
4. **The allowlist change reached the session proxy but not the container
   path.** The owner switched the Default environment to Custom network
   access with those four hosts plus the default list. The host (through
   the session's HTTPS proxy) reached all four within a minute, but
   containers on the default bridge network kept getting 403 from the
   egress gateway for the newly allowed hosts (pypi still worked) across
   ten checks 30 s apart. Fixed in the script: on a cloud host with
   `HTTPS_PROXY` set, image builds run with `--network host` and the proxy
   passed as build arguments, which Docker does not store in the image.
   apt, curl, git and pip all honour it. The task images were built the
   same way. Open item for a cloud gate run: Harbor's own image builds will
   need the same proxy route (or the container path must start honouring
   the environment's allowed domains).
5. **Two of the host's CA files have no trailing newline**
   (`swp-ca-production.crt`, `swp-ca-staging.crt`), so concatenating them
   glued two certificates onto one line and `keytool` rejected the bundle
   ("Input not an X.509 certificate"). The script now writes a newline
   after each file, and `Dockerfile.jvm` copies only BEGIN..END blocks.
6. **Docker does not survive a VM restore.** dockerd had to be restarted by
   hand twice in one session after the VM paused. Start it in the
   environment's setup script (or a SessionStart hook) before anything
   that uses images.

With 4 and 5 fixed, setup-host.sh ends with "frontier-v5 setup: done" on
the x86-64 VM and lists v5-base, v5-cfamily, v5-jvm and v5-rust (plus the
local trust layer v5-python-ca); Harbor 0.24.0 installed. Tool versions in
the amd64 tags match the table at the top (GCC 12.2.0-14+deb12u1, CMake
3.25.1, Temurin 21.0.12.1; the JVM image carries the seven interception
CAs in its cacerts).

## C++ pipeline proven 2026-10-07

tasks/frontier-v5/fmt-format-spec-conformance validates end to end (base
reward 0 x3, gold reward 1 x3, nine single-fix controls) on
`vulcanbench/sandbox:v5-cfamily` built on the x86-64 cloud VM, the first
Track A task in C or C++. Lessons for the cpp-v1 checklist:

- Pick the language standard in the grading image, not on the host. The
  first choice, C++20, built and passed with the host's GCC 13 but GCC 12.2
  cannot compile {fmt}'s existing `base-test.cc` in C++20 mode at the base;
  the task builds as C++17 (GCC 12's default, one of upstream's CI
  configurations). A hidden test that is a compile error at base also takes
  its whole test binary down with it, so prefer standards where the held-out
  tests compile at base and fail at run time.
- Run one googletest case per process. Debug builds keep `FMT_ASSERT`
  live, and one base failure (a calendar formatter fed a zeroed `tm`)
  aborts the process, which would hide every later case in that binary.
  `tests/run_tests.py` lists each ctest entry's cases and runs each with
  `--gtest_filter`; 584 cases take seconds.
- Take only the directories upstream ships from the agent's artifact, and
  check include order. The first verifier copied the agent's whole
  `include/`, which is an `-I` path searched before googletest's
  `-isystem` path, so an `include/gtest/gtest.h` that no-ops `EXPECT_*`
  passed every held-out case with no fix. Now only `include/fmt` is taken,
  and a shadowing probe is part of validation. The same class of hole is
  worth auditing in the other tasks: a test-framework class or module placed
  inside the graded tree (Java `src/main` precedes the JUnit jar on
  Maven's test classpath; a `conftest.py` inside a graded Python package is
  collected by pytest).
- googletest is vendored in the {fmt} tree, so the agent and verifier
  images build and test with no network at all; the verifier builds from
  scratch with pristine CMake files and test sources and grades
  `include/` and `src/` only.
- A test overlay copied with `cp -a` keeps old mtimes, and Ninja then
  skips the rebuild; irrelevant to the verifier (fresh build tree) but it
  silently fakes a base pass in an incremental host build.
- `long double` is 80-bit x87 on x86-64 and IEEE quad on arm64 Linux; one
  held-out case depends on it, so arm64 validation is a separate check
  before the task is called portable.

## Verifier audit 2026-10-08

Reviewing the fmt task before merge turned up a header-shadowing hole, so
every Track A verifier was attacked on purpose. Each attack below is a few
lines of graded source that fix nothing; "old" is the verifier as first
validated.

| Task | Attack | Old reward | Hole |
| --- | --- | --- | --- |
| fmt | `include/gtest/gtest.h` turning `EXPECT_*` into no-ops | f2p all green | agent `include/` is an `-I` path searched before googletest's `-isystem` |
| nx-digraph (also nx-group, sympy) | `os._exit(0)` in `__init__.py` when pytest is loaded | 1 | graded on pytest's exit status |
| nx-digraph | `conftest.py` beside the tests rewriting failed reports | 1 | agent `conftest.py` files were copied and loaded |
| nx-digraph | `importlib` patch of pytest's `TestReport`, no test-runner words | 1 (even with report grading) | code under test shares the pytest process |
| petgraph | `std::process::exit(0)` in the max-flow entry points | 1 | graded on `cargo test`'s exit status |
| luxon | `src/luxon.js` replacing the global `expect` with a no-op | f2p all green | code under test shares the jest worker |
| commons-lang | shadow `AssertionFailureBuilder` and opentest4j types in `src/main` | results changed | main classes precede the JUnit jars on the test classpath |

Every task now has three layers, and a new task must ship all three plus
these probes before it counts as validated:

1. **Structural.** Take only what upstream ships at the graded path:
   `include/fmt`, `src/main/java/org/apache/commons/lang3`, the package
   tree minus `tests/` and every `conftest.py`.
2. **Report-based grading.** Never trust the runner's exit status. Every
   expected id must appear as passed in a report the runner writes
   (JUnit XML, libtest `--logfile` at an unpredictable path, jest JSON,
   per-case gtest output, surefire XML), with the total pinned, so an early
   exit leaves tests missing and fails.
3. **Tamper detection.** `tests/tamper_scan.py` (one file, identical in
   every task) rejects added lines that reference the test framework or its
   report format, force an exit, or (C++) use `dlsym`/`dlopen`, and new
   files with a runner meaning. For in-process runners (pytest, jest,
   googletest) a sentinel test asserting something false is appended to the
   overlaid gold files and must be reported failed (`must_fail`), which
   catches obfuscated neutering the scan cannot see.

Each layer was shown to be needed: scan-evading exits (`getattr(os,
"_ex" + "it")`, `use std::process::{exit as quit}`, a reflected
`Runtime.halt`, a function pointer to `_exit`) pass the scan and are caught
by report grading; the pytest report patch and a `Function("return
this")` expect patch pass both the scan and report grading and are caught
only by the sentinels. After the change every task re-probed on the x86-64
cloud VM at base 0, gold 1, every attack 0 (records in each DESIGN.md).

Residual risk, stated plainly: a determined agent can still tamper in ways
none of this sees (for example neutering assertions only for test names it
guesses are hidden while leaving the sentinel alone). The gate should
therefore include a diff review of every solved run, looking for test
framework interaction and exits, before a solve counts against the 1/3
bar. The F1 scaffolds (OpenAPV, libmbus) were not audited here; that
belongs to the Track B session.

## C and F2 pipelines proven 2026-10-08

- **Plain C (CMake + ctest):** yyjson-incremental-and-mutation-fixes builds
  the pinned base's test suite with the fixed (gold) tests applied from a
  patch, grades per test executable from `ctest --output-junit` (not ctest's
  exit status), and adds a sentinel executable compiled against the agent's
  library that must abort. The writer-exit probe showed per-executable exit
  codes alone are foolable (a constructor `exit(0)` makes every executable
  exit 0, so ctest marks them passed); the tamper scan and the sentinel
  catch it. c-v1 checklist, this task its first entry.
- **F2 differential parity (Rust):** comrak-gfm-tables-autolinks-parity.
  Lessons: build the reference's expected outputs in a throwaway verifier
  stage pinned by output hash, keep them root-only, and run the agent's
  binary as `nobody` so it cannot read them; match the reference CLI's flags
  exactly (here `--syntax-highlighting none --gfm-quirks`), since a flag
  mismatch looks like thousands of "failures"; and source the whole corpus
  from the reference project's own example files plus seeded combinations,
  never hand-written payloads. A natural-drift F2 target did not work
  (comrak has matched cmark-gfm for years); removing two whole parsers gives
  an honest gap whose gold is real upstream code.
- **Verifier probe harness:** scripts/frontier-v5/probe_verifier.py turns the
  audit's attacks into a per-task gate (tests/probes.json). Required for
  every new task.

## First subscription gate attempt 2026-10-08: two grading bugs

The first real `harbor run` (codex:gpt-6-astra at medium, ChatGPT
subscription via `CODEX_FORCE_AUTH_JSON=1`, Codex CLI 0.159.0; 0.149.0
refuses the model) on nx-digraph-node-connectivity could not be scored:

1. **Wrong verifier entry point.** Harbor 0.24.0 runs the verifier image's
   own `/tests/test.sh` (finding 4), but every Track A tests/Dockerfile
   installed the script only at `/test.sh`, and probe_verifier.py called
   `/test.sh` directly, so no validation exercised Harbor's path. Harbor's
   `oracle` agent reproduced it on main: RewardFileNotFoundError, "bash:
   /tests/test.sh: No such file or directory". Each Dockerfile now also
   installs `/tests/test.sh`, and probe_verifier.py calls `/tests/test.sh`,
   so the two cannot drift again.
2. **Bytecode false positive.** An agent that runs pytest leaves
   `__pycache__/*.pyc` in the artifact, and the tamper scan flagged every one
   as a new binary file (287 findings on the Astra run), so any such agent
   scored 0. For Python only, the scan now skips the byproducts
   `harness.tasks.is_local_junk` skips (DECISIONS 2026-10-06), and the three
   Python verifiers no longer copy `__pycache__` or `*.py[co]` into the
   graded tree, so a planted bytecode file cannot stand in for its source.
   Checked on nx-digraph: gold plus 583 compiled caches scores 1 (it scored
   0 before); base plus forged gold bytecode stamped with the base sources'
   mtime and size scores 0; every existing probe still scores 0.

Rule for new tasks: validate through `harbor run -a oracle` (gold must score
1 under Harbor itself), not only probe_verifier.py.
