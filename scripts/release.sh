#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?usage: $0 <version like 0.1.1>}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must be x.y.z" >&2; exit 1; }
TAG="v$VERSION"

if ! git diff-index --quiet HEAD --; then
  echo "working tree not clean; commit or stash first" >&2
  exit 1
fi

sed -i -E "s/^version = \"[^\"]+\"/version = \"$VERSION\"/" Cargo.toml
cargo build --release --workspace --quiet
git add Cargo.toml
[ -f Cargo.lock ] && git add Cargo.lock
git commit -m "chore: release $TAG"
git tag -a "$TAG" -m "Release $TAG"
git push origin main
git push origin "$TAG"
echo "✓ released $TAG — CI will build and attach binaries"
