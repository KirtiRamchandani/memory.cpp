#!/usr/bin/env bash
set -euo pipefail
[[ -f website/index.html ]]
[[ -f website/styles.css ]]
[[ -f website/app.js ]]
while IFS= read -r href; do
  case "$href" in
    http*|mailto:*|#*|'') continue ;;
  esac
  path="website/$href"
  [[ -f "$path" ]] || { echo "missing website link target: $href" >&2; exit 1; }
done < <(grep -oE 'href="[^"]+"' website/index.html | sed 's/^href="//;s/"$//')
printf 'Website check passed.\n'