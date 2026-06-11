# Pull Request

## Summary
<!-- One paragraph: what + why -->

## Type of change
- [ ] Bug fix
- [ ] New feature / harness capability
- [ ] New seed task (see TASK_CONTRIBUTION checklist below)
- [ ] Documentation only
- [ ] Refactor / DX

## Checklist
- [ ] `make ci` (lint + type + fast tests) passes locally
- [ ] New code has tests (or existing coverage not regressed)
- [ ] Docs updated if user-facing
- [ ] No secrets or large binaries committed
- [ ] For tasks: gold-patch run 3× on ARM64 + x86, validation script output attached

## Related issues
Closes #

---

### For Task Submissions Only (PR to tasks/v1/)
- [ ] `python scripts/validate_tasks.py tasks/v1/<your-id>` passes
- [ ] `metadata.json` complete (decontamination_notes, cutoff dates per model)
- [ ] `ground_truth.patch` + `verifier.py` + 3× gold runs produce identical PASS/FAIL
- [ ] `repo_snapshot.tar.gz` tracked in Git LFS, < reasonable size
- [ ] Issue/PR source is post-cutoff for all listed models
- [ ] License of source repo compatible (permissive or equivalent)
- [ ] Added to this PR description: full output of validate + `vulcanbench validate-task ...`

Thank you! Your task will help the community.
