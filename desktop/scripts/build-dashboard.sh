#!/usr/bin/env bash
# Build the dashboard (Next.js, output: standalone) and assemble the files the
# app ships: desktop/resources/dashboard/{server.js,.next,node_modules,public}
# plus build-info.json naming the checkout the app launches.
#
# usage: bash desktop/scripts/build-dashboard.sh [--no-install]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # desktop/
repo="$(cd "$here/.." && pwd)"
dash="$repo/dashboard"
out="$here/resources/dashboard"
install=1
[[ "${1:-}" == "--no-install" ]] && install=0

if ! command -v npm >/dev/null 2>&1; then
  echo "npm not found; install node (brew install node@22) and put it on PATH" >&2
  exit 2
fi

cd "$dash"
if [[ $install -eq 1 ]]; then
  npm ci --no-audit --no-fund
fi
npm run build

if [[ ! -f .next/standalone/server.js ]]; then
  echo "expected .next/standalone/server.js; is next.config output set to standalone?" >&2
  exit 1
fi

rm -rf "$out"
mkdir -p "$out"
# The standalone tree is self-contained except for static assets and public/,
# which Next leaves for the deployer to copy (as documented).
cp -R .next/standalone/. "$out/"
mkdir -p "$out/.next"
cp -R .next/static "$out/.next/static"
[[ -d public ]] && cp -R public "$out/public"

commit="$(git -C "$repo" rev-parse --short HEAD 2>/dev/null || echo unknown)"
cat > "$here/resources/build-info.json" <<EOF
{
  "repoRoot": "$repo",
  "commit": "$commit",
  "builtAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

echo "dashboard assembled at $out ($(du -sh "$out" | cut -f1)); launches checkout $repo at $commit"
