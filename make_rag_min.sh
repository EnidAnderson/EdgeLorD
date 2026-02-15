#!/usr/bin/env bash
set -euo pipefail

RAG_SCRIPT="${RAG_SCRIPT:-../rag_semantic_bundle.py}"
OUT_DIR="${OUT_DIR:-artifacts}"
BASE_REF="${1:-origin/main}"

MAX_OUTPUT_KB="${MAX_OUTPUT_KB:-6000}"
MAX_FILES="${MAX_FILES:-3000}"
MAX_FILE_BYTES="${MAX_FILE_BYTES:-2000000}"
INCLUDE_PRIVATE="${INCLUDE_PRIVATE:-0}"

mkdir -p "$OUT_DIR"
ts="$(date -u +"%Y%m%dT%H%M%SZ")"
out="${OUT_DIR}/semantic_bundle_CHANGED_${BASE_REF//\//_}_${ts}.md"
latest="${OUT_DIR}/semantic_bundle_CHANGED_LATEST.md"

export RAG_BRIEF="${RAG_BRIEF:-1}"

args=( --mode changed --base "$BASE_REF" --include-untracked
       --max-files "$MAX_FILES" --max-file-bytes "$MAX_FILE_BYTES" --max-output-kb "$MAX_OUTPUT_KB"
       --exclude-glob ".cache/**" --exclude-glob "**/.cache/**"
       --exclude-glob "target/**" --exclude-glob "**/target/**"
       --exclude-glob ".direnv/**" --exclude-glob "**/.direnv/**"
       --exclude-glob "node_modules/**" --exclude-glob "**/node_modules/**"
       --exclude-glob "dist/**" --exclude-glob "**/dist/**"
       --exclude-glob "build/**" --exclude-glob "**/build/**"
)

if [[ "$INCLUDE_PRIVATE" == "1" ]]; then
  args+=( --include-private )
fi

python3 "$RAG_SCRIPT" "${args[@]}" > "$out"

cp -f "$out" "$latest"

echo "Base:  $BASE_REF"
echo "Wrote: $out"
echo "Latest: $latest"
