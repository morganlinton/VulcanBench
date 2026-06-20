FROM python:3.12-slim-bookworm

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*

COPY pyproject.toml README.md LICENSE NOTICE ./
COPY harness harness
COPY backend backend
COPY alembic alembic
COPY alembic.ini ./

RUN pip install --no-cache-dir -e ".[backend]"

ENV VULCANBENCH_RUNS_DIR=/data/runs \
    VULCANBENCH_TASKS_ROOT=/app/tasks/v1

COPY tasks/v1 tasks/v1

RUN mkdir -p /data/runs

EXPOSE 8000

CMD ["sh", "-c", "alembic upgrade head && uvicorn backend.app:app --host 0.0.0.0 --port 8000"]
