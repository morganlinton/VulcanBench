# Separate verifier image. Build context is tests/, so the hidden tests are
# baked in here; Harbor skips the tests upload whenever tests/Dockerfile exists.
FROM python:3.12-slim-bookworm
COPY . /tests
RUN chmod +x /tests/test.sh
