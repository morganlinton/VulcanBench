# VulcanBench Makefile - primary developer interface
SHELL := /bin/bash
PYTHON := python3
VENV := .venv
VENV_BIN := $(VENV)/bin
PIP := $(VENV_BIN)/pip
PYTEST := $(VENV_BIN)/pytest
RUFF := $(VENV_BIN)/ruff
MYPY := $(VENV_BIN)/mypy
VULCANBENCH := $(VENV_BIN)/vulcanbench

# Put the venv's bin on PATH for every recipe, so tools the tests shell out to
# (python -m pytest, ruff, bandit, radon, go, node) resolve without activating
# the venv. This makes `make ci` behave the same from any shell.
export PATH := $(abspath $(VENV_BIN)):$(PATH)

.PHONY: help setup clean install dev test lint typecheck fmt ci docker-up docker-down validate-tasks sandbox-image
.DEFAULT_GOAL := help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

setup: $(VENV)/touch ## One-command setup: create venv, install editable + dev deps (run once after clone)
	@echo "✅ VulcanBench dev environment ready. Activate with: source $(VENV)/bin/activate"
	@echo "   Then: vulcanbench --help"
	@echo "   Dashboard: cd dashboard && npm install && npm run dev"

$(VENV)/touch: pyproject.toml
	$(PYTHON) -m venv $(VENV)
	$(PIP) install --upgrade pip setuptools wheel
	$(PIP) install -e ".[dev,test]"
	@touch $(VENV)/touch

clean: ## Remove venv, build artifacts, runs (keep source)
	rm -rf $(VENV) .ruff_cache .mypy_cache .pytest_cache __pycache__ dist build *.egg-info runs/* || true
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true

install: setup ## Alias for setup

dev: setup ## Install in editable mode (already done in setup)

test: setup ## Run fast unit tests (target >=80% coverage on harness)
	$(PYTEST) -m "not slow and not docker" --cov=harness

test-all: setup ## Run all tests including slow/Docker (CI)
	$(PYTEST)

lint: setup ## Ruff lint + format check (zero warnings enforced)
	$(RUFF) check .
	$(RUFF) format --check .

fmt: setup ## Auto-format with ruff
	$(RUFF) format .
	$(RUFF) check --fix .

typecheck: setup ## Strict mypy
	$(MYPY) harness backend scripts/ingest_runs.py

ci: lint typecheck test ## Full local CI (lint + types + fast tests)

sandbox-image: ## Build the Docker sandbox image (runs use it by default)
	docker build -t vulcanbench/sandbox:base -f sandbox/Dockerfile.base .
	@echo "✅ Built vulcanbench/sandbox:base — vulcanbench run uses it by default"

docker-up: ## Start local stack (postgres, minio, later dashboard+backend)
	docker compose up -d --build

docker-down: ## Stop local stack
	docker compose down -v

vulcanbench-version: setup ## Smoke: verify CLI entrypoint
	$(VULCANBENCH) --version

dashboard-dev: ## Start Next.js dashboard (assumes npm install done)
	cd dashboard && npm run dev

validate-tasks: setup ## Validate all task definitions (gold-solves, fail-to-pass, determinism)
	$(VENV_BIN)/python scripts/validate_tasks.py tasks/v1
