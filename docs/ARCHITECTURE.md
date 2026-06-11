# ARCHITECTURE.md (v1)

See the authoritative implementation plan: `.kilo/plans/1780024121334-silent-circuit.md` (internal, contains full research, dataflow, risks, phases).

## High-Level (from plan section 2)

```
VulcanBench/
├── harness/          # Python CLI + engine (Typer, Pydantic v2, Docker)
│   ├── cli.py
│   ├── agent/        # Tool ABC + executors (Docker + Local for test)
│   ├── sandbox/      # DockerSandbox (pinned multi-arch images, non-root, limits)
│   ├── evaluator/    # functional + quality + security + efficiency + human_like
│   └── tracer/       # JSONL + self-contained replay.html
├── tasks/v1/         # 5-10 seed tasks (hand-curated, gold validated 3×)
├── dashboard/        # Next.js 15/16 App Router (leaderboard, trace viewer)
├── backend/          # FastAPI + SQLModel + Postgres (ingest + queries)
├── sandbox/          # Dockerfile.base + task template
├── scripts/          # validate_task.py etc.
└── docker-compose.yml (postgres + minio + future services)
```

## Data Flow (one eval run)
1. CLI `run` → resolves task metadata + snapshot
2. Sandbox launches isolated container (repo at /workspace/repo RO, patch area RW)
3. Agent loop (built-in minimal ReAct or external via stdio/HTTP protocol) calls tools
4. Every event (llm_*, tool_*, file_edit as unified diff, cmd, test) → append-only trace.jsonl
5. On done: verifier + linters + scans → scores → summary.json + final.patch + replay.html
6. Persist to local runs/ + POST to backend API (optional)
7. Leaderboard aggregates

## Tool Contract
Standardized across local/Docker/replay:
- list_files, read_file, search_code (ripgrep), edit_file (strict), run_command, run_tests etc.
- Exposed as OpenAI function-calling schema for compatibility.

## Reproducibility
- Every trace embeds: image digests, docker-compose, CLI cmd, git commit of harness
- `vulcanbench replay --run-id X` reconstructs + re-executes (or fast-forwards)
- Gold patches validated 3× on ARM64 + x86_64

## Non-Functional (enforced)
- Strict typing (Pydantic + mypy), ruff zero-warn, >=80% coverage on harness core
- Non-root containers, .dockerignore strict, no secrets
- Self-contained HTML replays

Full details, decision records, and exact metric formulas in the plan + future METRICS.md / REPRODUCIBILITY.md.
