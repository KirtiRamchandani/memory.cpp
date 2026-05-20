#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=0
REPO="${MEMORY_CPP_REPO:-KirtiRamchandani/memory.cpp}"
VERSION="${MEMORY_CPP_VERSION:-latest}"
BIN_DIR="${MEMORY_CPP_BIN_DIR:-$HOME/.local/bin}"

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --help|-h)
      echo "usage: install.sh [--dry-run]"
      exit 0
      ;;
    *) echo "unknown argument: $arg" >&2; exit 1 ;;
  esac
done

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"
case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
esac

case "$os" in
  linux) platform="linux" ;;
  darwin) platform="macos" ;;
  msys*|mingw*|cygwin*) platform="windows" ;;
  *) platform="$os" ;;
esac

asset="memory-${platform}-${arch}"
if [ "$platform" = "windows" ]; then
  asset="${asset}.zip"
else
  asset="${asset}.tar.gz"
fi

echo "memory.cpp installer"
echo "repo: $REPO"
echo "target: $platform/$arch"
echo "bin dir: $BIN_DIR"

if [ "$DRY_RUN" = "1" ]; then
  echo "dry run: would try GitHub release asset $asset, verify checksum if present, then fall back to cargo install."
  echo "next after install: memory setup --developer --yes"
  exit 0
fi

mkdir -p "$BIN_DIR"

install_from_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo was not found. Install Rust from https://rustup.rs/ or download a release binary." >&2
    exit 1
  fi
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/.." && pwd)"
  cargo_root="$(dirname "$BIN_DIR")"
  cargo install --path "$repo_root/crates/memory-cli" --force --root "$cargo_root"
}

download_release() {
  if ! command -v curl >/dev/null 2>&1; then
    return 1
  fi
  tmp="$(mktemp -d)"
  if [ "$VERSION" = "latest" ]; then
    url="https://github.com/${REPO}/releases/latest/download/${asset}"
    checksum_url="https://github.com/${REPO}/releases/latest/download/checksums.txt"
  else
    url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
    checksum_url="https://github.com/${REPO}/releases/download/${VERSION}/checksums.txt"
  fi
  echo "trying release asset: $url"
  if ! curl -fsSL "$url" -o "$tmp/$asset"; then
    rm -rf "$tmp"
    return 1
  fi
  if curl -fsSL "$checksum_url" -o "$tmp/checksums.txt"; then
    if command -v sha256sum >/dev/null 2>&1; then
      (cd "$tmp" && grep " $asset\$" checksums.txt | sha256sum -c -) || {
        echo "checksum verification failed" >&2
        rm -rf "$tmp"
        return 1
      }
      echo "checksum verified"
    fi
  else
    echo "checksum file unavailable; continuing without checksum verification"
  fi
  if [[ "$asset" == *.zip ]]; then
    command -v unzip >/dev/null 2>&1 || return 1
    unzip -q "$tmp/$asset" -d "$tmp/out"
  else
    tar -xzf "$tmp/$asset" -C "$tmp"
    mkdir -p "$tmp/out"
    find "$tmp" -maxdepth 2 -type f -name "memory*" -exec cp {} "$tmp/out/" \;
  fi
  binary="$(find "$tmp/out" -type f -name 'memory*' | head -n 1)"
  if [ -z "$binary" ]; then
    rm -rf "$tmp"
    return 1
  fi
  cp "$binary" "$BIN_DIR/memory"
  chmod +x "$BIN_DIR/memory"
  rm -rf "$tmp"
}

if ! download_release; then
  echo "release binary unavailable; falling back to cargo install"
  install_from_cargo
fi

echo
echo "Installed memory.cpp."
echo "If needed, add this to PATH:"
echo "  export PATH=\"$BIN_DIR:\$PATH\""
echo
echo "Try:"
echo "  memory welcome"
echo "  memory setup --developer --yes"
echo "  memory doctor"
