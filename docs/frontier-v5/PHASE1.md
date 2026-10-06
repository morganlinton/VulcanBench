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
