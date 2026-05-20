#!/usr/bin/env bash
set -euo pipefail
find examples -maxdepth 1 -type f | sort
printf 'Examples are static docs. To refresh live examples, run scripts/demo.sh and copy concise output back intentionally.\n'