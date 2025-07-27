#!/bin/sh

set -e

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/..

cd "$ROOT"

if [ -z "$1" ]; then
	echo "usage: $0 crate..."
	exit 1
fi

cargo -q build
TARGET=$(realpath target/debug/cargo-lock-fetch)

D=$(mktemp -d)
cleanup() {
    rm -rf "$D"
}
trap cleanup EXIT

mkdir "$D/package"

cd "$D/package"
cargo -q init . --vcs none --name package
cargo -q add "$@"

cd - > /dev/null
export CARGO_HOME="$D/cargo_home"
$TARGET lock-fetch -q --lockfile-path "$D/package/Cargo.lock"
cd "$D/package"

if cargo -q fetch --frozen; then
	echo "pass: fetched $(find "$D/cargo_home/registry/cache/" -maxdepth 2 -type f | wc -l) crates"
else
	echo fail
	exit 1
fi
