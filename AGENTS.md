# AGENTS.md

VulcanBench is an open-source LLM benchmarking harness. It has three parts:

- **CLI harness** (`vulcanbench`, package `harness/`) — the core product; runs
  benchmark tasks and writes results under `./runs/`.
- **Backend API** (`backend/app.py`, FastAPI) — serves run data at `/api/*`.
- **Dashboard** (`dashboard/`, Next.js) — reads the API and renders the
  leaderboard / tasks / run traces.

Standard commands live in the `Makefile` (`make help`), `README.md`,
`docs/QUICKSTART.md`, `CONTRIBUTING.md`, and `dashboard/README.md`.

## Cursor Cloud specific instructions

The update script already installs Python deps (`.venv` via `make setup` plus the
`[backend]` extra) and dashboard deps (`npm ci` in `dashboard/`). Activate the
venv with `source .venv/bin/activate` before using `vulcanbench`.

- **Run Python tests via `make test` (or `make ci`), not `.venv/bin/pytest`
  directly.** The harness and several tests shell out to `ruff`, `bandit`, and
  `pytest` by name. The `Makefile` puts `.venv/bin` on `PATH`; invoking the venv
  binary directly instead leaves those off `PATH`, so many tests skip
  ("ruff not installed" / "pytest not on PATH") and coverage drops below the 80%
  gate — a false failure. With `PATH` set correctly the full fast suite passes
  (~83% coverage).
- **Docker is not installed in this environment.** Real model runs and the
  `--sandbox docker` default require it, as do `make sandbox-image*` and
  `make validate-tasks-docker`. For offline work use the deterministic mock model
  with the local sandbox:
  `vulcanbench run --task hello-world --model mock:synthetic --sandbox local`.
  Real providers (`openai:`, `anthropic:`, `zai:`, `kimi:`) additionally need the
  matching API key (`OPENAI_API_KEY`, etc.); none are set here.
- **Backend runs DB-free by default.** `uvicorn backend.app:app --port 8000`
  reads `./runs/` from the filesystem (`/api/health` reports
  `"store":"filesystem"`); no Postgres needed. A DB is only used when
  `DATABASE_URL` is set (optional; `docker compose up db` needs Docker).
- **Dashboard** (`cd dashboard && npm run dev`, port 3000) fetches the backend at
  `http://localhost:8000` by default and shows an offline banner if it is down,
  so start the backend first. Note: deleting/recreating `dashboard/node_modules`
  while `npm run dev` is running breaks the live server — restart it afterward.
- **Lint/typecheck currently fail on pre-existing issues, not env problems.**
  `ruff`/`mypy` are unpinned (`>=`), so the newest versions install; against them
  `main` already fails `ruff check` (import order in `tests/test_providers.py`)
  and `mypy` (unused `type: ignore` in `alembic/env.py`). The tooling itself
  works — these are pre-existing repo lint failures.
