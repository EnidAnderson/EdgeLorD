#!/usr/bin/env bash
set -euo pipefail

RAG_BRIEF=0 python3 ../rag_semantic_bundle.py --mode overview --include-untracked --include-private \
  --max-files 1000000 --max-file-bytes 5000000 --max-output-kb 200000 \
  --exclude-glob ".cache/**" --exclude-glob "**/.cache/**" \
  --exclude-glob "target/**" --exclude-glob "**/target/**" \
  --exclude-glob ".direnv/**" --exclude-glob "**/.direnv/**" \
  --exclude-glob "node_modules/**" --exclude-glob "**/node_modules/**" \
  --exclude-glob "dist/**" --exclude-glob "**/dist/**" \
  --exclude-glob "build/**" --exclude-glob "**/build/**" \
  > semantic_bundle_MAX.md

echo "Wrote: semantic_bundle_MAX.md"
