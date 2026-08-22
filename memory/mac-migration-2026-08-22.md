---
name: mac-migration-2026-08-22
description: Dev moved from the Win11 box to a Mac (arm64) on 2026-08-22 — environment rebuilt, vendor-defect verification + CI format fix shipped (PRs #48/#49)
metadata:
  type: project
---

Development moved off the Win11 box to a macOS arm64 machine on 2026-08-22. What happened and what shipped:

- **Environment rebuilt, all user-level (no Homebrew/sudo):** Python 3.12.14 via `uv`, git-lfs 3.7.1, rustup/cargo, Go 1.27.0, Node v24 LTS, and `gh` 2.98.0 in `~/.local/bin` / `~/.cargo` / `~/.local/{go,node}`, persisted via `~/.zshrc`. Docker Desktop was pre-installed; sandbox images `base`, `rust`, `rust-2024`, `go-1.26` built via `make sandbox-image-all` (per-task images left to build on demand). Verified: `make test` 576 passed; mock run in Docker sandbox scores functional=1.0. No provider API keys on this machine yet.
- **git-lfs incident:** the repo arrived with a completely empty git index (~25k phantom staged deletions) because LFS-filtered operations had failed with git-lfs missing. Repaired with a filter-bypassed `git reset`; once git-lfs was installed the tree was fully clean — the working-tree tarballs matched their pointers exactly.
- **PR #48 (merged, `a57af06a`):** closed out the Go `vendor/` clean-clone defect from [[vulcancyber-suite-health]]. The code fixes had already landed (45e79f03, 1cc69d45); #48 added the verification (clean `git archive` checkouts build offline with `GOPROXY=off -mod=vendor`; both tasks re-pass Docker validation gold=1.0 deterministic ×3; corpus-wide sweep found no other vendor mismatches) plus `tests/test_slice_repo_vendor.py` (12 regression tests for `_untrack_vendor_in_gitignore`).
- **PR #49 (merged, `40c4d693`):** `main` CI had been red since the direct push `006418e6` — `scripts/mine_oss_prs.py` landed unformatted and the `ruff format --check` gate failed before mypy/pytest ran. Mechanical reformat; CI on main green again as of run 32583594580.

Watch-out that caused #49: direct pushes to main skip the PR checks, and the format gate fails the whole job first — run `make lint` (or `make ci`) before any direct push.
