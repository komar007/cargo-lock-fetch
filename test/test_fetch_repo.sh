#!/bin/sh

set -e

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/..

if [ -z "$1" ]; then
	echo "usage: $0 repository [commit]..."
	exit 1
fi

cd "$ROOT"
cargo -q build
cd - >/dev/null

TARGET=$(realpath target/debug/cargo-lock-fetch)

D=$(mktemp -d)
cleanup() {
	rm -rf "$D"
}
trap cleanup EXIT

git clone -q "$1" "$D/crate"
cd "$D/crate"
if [ -n "$2" ]; then
	git checkout -q "$2"
fi
cd - >/dev/null

export CARGO_HOME="$D/cargo_home"
$TARGET lock-fetch -q --lockfile-path "$D/crate/Cargo.lock"
cd "$D/crate"

if cargo -q fetch --frozen; then
	echo "pass: fetched $(find "$D/cargo_home/registry/cache/" -maxdepth 2 -type f | wc -l) crates"
else
	echo fail
	exit 1
fi
