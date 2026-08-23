# AGENTS.md

## Cursor Cloud specific instructions

VulcanBench is a Python 3.12 LLM benchmarking harness. Standard commands live in
the root `Makefile` and `README.md` (`make help` lists everything). This section
only captures non-obvious caveats for working in the cloud environment.

### Services

- `vulcanbench` CLI (core product, Python). Activate the venv first:
  `source .venv/bin/activate`, then `vulcanbench --help`.
- FastAPI backend (`backend/app.py`), optional — powers the dashboard.
- Next.js dashboard (`dashboard/`), optional web UI. See `dashboard/AGENTS.md`
  (this is Next.js 16 with breaking changes vs. common knowledge).

### Non-obvious caveats

- The update script already creates `.venv` and installs the harness in editable
  mode. Activate it with `source .venv/bin/activate` before running `vulcanbench`,
  `pytest`, `ruff`, or `mypy`. The `Makefile` targets also work without activation
  because they put `.venv/bin` on `PATH`.
- The venv is installed with the `dev,test,backend` extras (not just `dev,test`
  from `make setup`). The `backend` extra (fastapi/uvicorn/sqlmodel) is required
  to run the API with `uvicorn backend.app:app --port 8000`.
- Docker is NOT available in this environment. Real benchmark runs default to
  `--sandbox docker` and will error out. For offline testing use the local
  sandbox + mock provider: `vulcanbench run --task hello-world --model mock:synthetic --sandbox local`.
  Consequently `make sandbox-image*`, `make docker-up` (Postgres), and
  `validate-tasks-docker`/`validate-cyber` cannot run here without a Docker daemon.
- The backend serves the filesystem `./runs/` directory by default (store =
  `filesystem`, no DB needed). Health check: `GET /api/health`. Postgres/`DATABASE_URL`
  is only needed for the durable DB store and its write endpoints.
- The dashboard needs `dashboard/.env.local` (copy from `dashboard/.env.example`);
  `NEXT_PUBLIC_API_BASE` must point at the backend (default `http://localhost:8000`).
  Start it with `cd dashboard && npm run dev` (serves `http://localhost:3000`).
- `make lint` (ruff) and the fast test suite are clean. `make typecheck` (mypy)
  can report a spurious `unused-ignore` in `alembic/env.py` because mypy is not
  pinned and a newer version resolves the ignored import differently — this is
  environment version drift, not a code defect.
- The fast test suite (`make test` / `pytest -m "not slow and not docker"`) takes
  roughly 6 minutes and enforces >=80% coverage on `harness`.
