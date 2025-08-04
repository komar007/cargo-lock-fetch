#!/usr/bin/env bash

set -e

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/..

VERSION=$(grep version "$ROOT/Cargo.toml" | cut -f 2 -d\")

if ! grep -qF "komar007/cargo-lock-fetch:$VERSION" "$ROOT/examples/fetch-deps-to-layer-from-docker/Dockerfile"; then
	echo "bad version in fetch-deps-to-layer-from-docker"
	exit 1
fi
if ! grep -qF "cargo-lock-fetch@$VERSION" "$ROOT/examples/fetch-deps-to-layer/Dockerfile"; then
	echo "bad version in fetch-deps-to-layer"
	exit 1
fi
