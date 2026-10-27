.PHONY: install up down test lint migrate run clean

VENV ?= .venv
PYTHON ?= $(VENV)/bin/python
PIP ?= $(VENV)/bin/pip

install:
	python3 -m venv $(VENV)
	$(PIP) install --upgrade pip
	$(PIP) install -e ".[dev]"

up:
	docker compose up -d --build

down:
	docker compose down -v

test:
	$(PYTHON) -m pytest tests/ -v

lint:
	$(PYTHON) -m ruff check .

migrate:
	$(PYTHON) -m alembic upgrade head

run:
	$(PYTHON) -m uvicorn app.main:app --host 0.0.0.0 --port 8000 --reload

clean:
	rm -rf .pytest_cache .ruff_cache __pycache__ app/**/__pycache__ tests/**/__pycache__
