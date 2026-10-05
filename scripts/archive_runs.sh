#!/usr/bin/env bash
# Archive a finished run directory outside git, and leave a pointer behind.
#
# Raw run output (traces, agent session logs, workspaces) never goes in the
# repo; see docs/HOW_WE_WORK.md. This copies a run directory to the archive
# named by VULCANBENCH_ARCHIVE and writes ARCHIVED.json into it recording where
# it went, so card scripts keep reading the local copy and anyone can find the
# canonical one later.
#
#   export VULCANBENCH_ARCHIVE=gs://<bucket>/vulcanbench    # or s3://..., or a path
#   bash scripts/archive_runs.sh runs-effort-grok47-cursor
#   bash scripts/archive_runs.sh runs-effort-grok47-cursor grok47-cursor-frontier-v4
#
# The upload is a sync, so re-running after more runs land is safe.
set -euo pipefail

src=${1:-}
name=${2:-}
if [[ -z "$src" || ! -d "$src" ]]; then
  echo "usage: $0 <run-dir> [archive-name]" >&2
  exit 2
fi
if [[ -z "${VULCANBENCH_ARCHIVE:-}" ]]; then
  echo "set VULCANBENCH_ARCHIVE (gs://bucket/prefix, s3://bucket/prefix, or a directory)" >&2
  exit 2
fi
src=$(cd "$src" && pwd)
name=${name:-$(basename "$src")}
dest="${VULCANBENCH_ARCHIVE%/}/$name"

case "$dest" in
  gs://*) gcloud storage rsync --recursive "$src" "$dest" ;;
  s3://*) aws s3 sync "$src" "$dest" ;;
  *)
    mkdir -p "$dest"
    if command -v rsync >/dev/null 2>&1; then rsync -a "$src/" "$dest/"; else cp -R "$src/." "$dest/"; fi
    ;;
esac

n_runs=$(find "$src" -name summary.json | wc -l | tr -d ' ')
bytes=$(du -sk "$src" | cut -f1)
cat > "$src/ARCHIVED.json" <<EOF
{
  "archived_to": "$dest",
  "archived_at": "$(date -u +%FT%TZ)",
  "n_run_summaries": $n_runs,
  "size_kib": $bytes
}
EOF
echo "archived $src -> $dest ($n_runs run summaries, ${bytes} KiB)"
