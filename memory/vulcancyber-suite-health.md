---
name: vulcancyber-suite-health
description: VulcanCyber v1 suite-integrity findings from the 2026-08-16 full Docker re-validation
metadata:
  type: project
---

Full `validate_tasks.py --sandbox docker` over all 17 tasks (2026-08-16, overnight) — every task is functionally sound (`gold=1.0, pre-patch=0.0, deterministic ×3`) — but the pass required fixing two things:

- **RESOLVED 2026-08-22 — Go tasks' `vendor/` defect is fixed and verified.** Commits `45e79f03` (vendor trees committed: echo 140 files, gosec 2389) and `1cc69d45` (`slice_repo.py` now strips vendor rules from sliced `.gitignore`s) landed on main. Verified on the Mac: clean `git archive` checkouts of both repos build offline (`GOPROXY=off go build -mod=vendor ./...`), both tasks re-pass full Docker validation (gold=1.0, pre-patch=0.0, deterministic ×3), a corpus-wide sweep found no other task with an untracked or gitignored vendor dir (the third `-mod=vendor` task, `v3/oss-cobra-noduplicateargs`, tracks 92 vendor files), and `tests/test_slice_repo_vendor.py` now regression-tests the gitignore-stripping helper. Original finding kept below for history.
- **HISTORICAL — Go tasks' `vendor/` is gitignored and never committed.** `oss-echo-encoded-path-separator` and `oss-gosec-g404-weak-random-coverage` carry `vendor` in their sliced `repo/.gitignore` → **0 tracked vendor files**, yet their `setup` is `go build -mod=vendor ./...`. So a **clean clone cannot build them offline**; the original admission only passed because the author's box had an uncommitted local `vendor/`. Rust tasks commit vendor correctly (gix 359, quick-xml 55; toml is zero-dep). Fix (a repo change, left for Morgan): regenerate (`go mod vendor` in each `repo/`) then `git add -f .../repo/vendor`, and audit `scripts/slice_repo.py` so Go slices force-add vendor. Proven sound: after `go mod vendor` (echo 140, gosec 2391 files) both pass ×3. Reported in `tasks/vulcancyber-v1/OVERNIGHT_REPORT_2026-08-16.md` (untracked).
- **BENIGN — Windows CRLF.** This box defaulted `core.autocrlf=true`; checkout rewrote LF→CRLF, breaking cargo vendored-source **checksums** (Rust) and `gold_patch.diff` apply. Committed blobs are LF (`git ls-files --eol` → `i/lf`), so CI/Linux is unaffected. Fixed repo-local: `git config --local core.autocrlf false && git config --local core.eol lf` + re-checkout. Durable option: a root `.gitattributes` (`tasks/** text=auto eol=lf`).

All 5 sandbox images now built locally (base, go-1.26, rust, node-ts, node-dompurify). Determinism gate = `DETERMINISM_RUNS = 3` in `harness/validate.py`. See [[vulcancyber-build-toolchain]] for the Windows validate gotchas (UTF-8 console, getuid guard) and [[vulcancyber-hard-tail-plan]] for the task-building status.
