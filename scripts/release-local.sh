#!/usr/bin/env bash
set -euo pipefail
mkdir -p dist
cargo build --release -p memory-cli
cp target/release/memory dist/memory
( cd dist && sha256sum memory > checksums.txt )
printf 'Local release artifact: dist/memory\nChecksum: dist/checksums.txt\n'