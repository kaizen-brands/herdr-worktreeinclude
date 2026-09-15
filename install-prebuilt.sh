#!/usr/bin/env bash
# herdr `[[build]]` step: download the prebuilt binary for this platform from the
# matching GitHub Release into bin/. Runs on `herdr plugin install`.
# `herdr plugin link` skips build — for local checkout see README.
set -euo pipefail

NAME="herdr-worktreeinclude"
REPO="kaizen-brands/herdr-worktreeinclude"

ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="$ROOT/bin"

# The release tag matches the manifest version, so a checkout always pulls its own release.
VERSION="$(grep -m1 '^version' "$ROOT/herdr-plugin.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
TAG="v${VERSION}"

os="$(uname -s)"
arch="$(uname -m)"
case "$os-$arch" in
  Darwin-arm64)                target="aarch64-apple-darwin" ;;
  Darwin-x86_64)               target="x86_64-apple-darwin" ;;
  Linux-aarch64 | Linux-arm64) target="aarch64-unknown-linux-musl" ;;
  Linux-x86_64)                target="x86_64-unknown-linux-musl" ;;
  *)
    echo "$NAME: no prebuilt binary for $os-$arch" >&2
    echo "$NAME: build from source: cargo build --release && mkdir -p bin && cp target/release/$NAME bin/" >&2
    exit 1
    ;;
esac

archive="${NAME}-${target}.tar.gz"
checksum="${NAME}-${target}.sha256"
base="https://github.com/${REPO}/releases/download/${TAG}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# GitHub release CDN can 404 briefly after publish; retry including on 404.
dl() { curl -fsSL --retry 5 --retry-delay 3 --retry-all-errors --retry-connrefused "$1" -o "$2"; }

echo "$NAME: downloading $archive ($TAG)"
if ! dl "$base/$archive" "$tmp/$archive"; then
  echo "$NAME: failed to download $base/$archive" >&2
  echo "$NAME: build from source: cargo build --release && mkdir -p bin && cp target/release/$NAME bin/" >&2
  exit 1
fi
dl "$base/$checksum" "$tmp/$checksum"

echo "$NAME: verifying checksum"
expected="$(awk '{print $1}' "$tmp/$checksum")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$archive" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "$tmp/$archive" | awk '{print $1}')"
fi
if [ "$expected" != "$actual" ]; then
  echo "$NAME: checksum mismatch (expected $expected, got $actual)" >&2
  exit 1
fi

mkdir -p "$BIN_DIR"
tar -xzf "$tmp/$archive" -C "$tmp"
# Archive may contain the binary at top level or under a folder.
if [ -f "$tmp/$NAME" ]; then
  src="$tmp/$NAME"
else
  src="$(find "$tmp" -type f -name "$NAME" | head -n1)"
fi
if [ -z "${src:-}" ] || [ ! -f "$src" ]; then
  echo "$NAME: binary not found inside archive" >&2
  exit 1
fi
install -m 0755 "$src" "$BIN_DIR/$NAME"
echo "$NAME: installed $BIN_DIR/$NAME"
