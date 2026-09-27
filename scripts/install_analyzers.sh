#!/usr/bin/env bash
# Install the pinned static analyzers the quality and security metrics use for
# JavaScript, C and C++ (harness/evaluator/quality.py, security.py, cfamily.py).
# Idempotent. Python (ruff, radon, bandit) comes from the project venv and Rust
# (cargo fmt, clippy, cargo-audit) from rustup.
#
#   bash scripts/install_analyzers.sh
#
# Versions are pinned so every run is scored by the same rules; each analyzer
# records its version in the run's score details. Bumping a version is a
# scoring change: log it in docs/DECISIONS.md.
set -euo pipefail

ESLINT_VERSION=10.11.0
ESLINT_JS_VERSION=10.0.1
ESLINT_PLUGIN_SECURITY_VERSION=4.0.1
CPPCHECK_VERSION=2.22.0
LLVM_MAJOR=23

ESLINT_DIR="${VULCANBENCH_ESLINT_DIR:-$HOME/.local/vulcanbench-eslint-$ESLINT_VERSION}"

# ESLint and plugins, in their own prefix so no workspace or global install is used.
if [ ! -x "$ESLINT_DIR/node_modules/.bin/eslint" ] ||
  [ "$("$ESLINT_DIR/node_modules/.bin/eslint" --version)" != "v$ESLINT_VERSION" ]; then
  mkdir -p "$ESLINT_DIR"
  (
    cd "$ESLINT_DIR"
    [ -f package.json ] || npm init -y >/dev/null
    npm install --save-exact --no-fund --no-audit \
      "eslint@$ESLINT_VERSION" "@eslint/js@$ESLINT_JS_VERSION" \
      "eslint-plugin-security@$ESLINT_PLUGIN_SECURITY_VERSION"
  )
fi
echo "eslint: $("$ESLINT_DIR/node_modules/.bin/eslint" --version) in $ESLINT_DIR"

# clang-tidy (Homebrew LLVM, keg-only so Apple clang stays the default compiler)
# and cppcheck. Homebrew installs the current formula; the check below fails
# loudly if it is not the pinned version.
if command -v brew >/dev/null 2>&1; then
  brew list cppcheck >/dev/null 2>&1 || brew install cppcheck
  brew list llvm >/dev/null 2>&1 || brew install llvm
fi
have_cppcheck=$(cppcheck --version | awk '{print $2}')
have_llvm=$(/opt/homebrew/opt/llvm/bin/clang-tidy --version | awk '/LLVM version/ {print $NF}')
echo "cppcheck: $have_cppcheck (pinned $CPPCHECK_VERSION)"
echo "clang-tidy: $have_llvm (pinned major $LLVM_MAJOR)"
if [ "$have_cppcheck" != "$CPPCHECK_VERSION" ] || [ "${have_llvm%%.*}" != "$LLVM_MAJOR" ]; then
  echo "analyzer versions differ from the pinned ones; scores may not compare" >&2
  exit 1
fi
