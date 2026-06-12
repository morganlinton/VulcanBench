# VulcanBench Dashboard

Next.js frontend for the VulcanBench API (`backend/app.py`): the model
leaderboard (with effort and complexity filters), per-task calibration views,
run traces, and the task-submission flow.

## Development

```bash
# 1. From the repo root, start the backend (serves ./runs at :8000)
uvicorn backend.app:app --port 8000

# 2. Run the dashboard
cd dashboard
npm install
npm run dev          # http://localhost:3000
```

Pages render server-side from live API data. When the backend is unreachable,
the leaderboard and tasks pages show an offline banner instead of silently
rendering empty data.

## Configuration

Copy `.env.example` to `.env.local` and adjust as needed:

| Variable               | Default                       | Purpose                              |
| ---------------------- | ----------------------------- | ------------------------------------ |
| `NEXT_PUBLIC_API_BASE` | `http://localhost:8000`       | Backend API base URL                 |
| `NEXT_PUBLIC_REPO`     | `Zen-Open-Source/VulcanBench` | GitHub repo for `/submit` and links  |

## Production

```bash
npm run lint
npm run build
npm start
```
