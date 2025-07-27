#!/bin/sh

set -e

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/..

cd "$ROOT"

if [ -z "$1" ]; then
	echo "usage: $0 crate"
	exit 1
fi

D=$(mktemp -d)
cleanup() {
    rm -rf "$D"
}
trap cleanup EXIT
mkdir "$D/fake"

cd "$D/fake"
cargo -q init . --vcs none --name compare
cargo -q add "$@"
cargo -q vendor "../vendor.expected/" --versioned-dirs
mv Cargo.lock ../
cd - > /dev/null
rm -fr "$D/fake"

cargo -q run -- lock-fetch -q --lockfile-path "$D/Cargo.lock" --vendor "$D/vendor.actual/" --versioned-dirs

if diff -Naur "$D/vendor.expected/" "$D/vendor.actual/" > /dev/null; then
	echo "pass: vendored $(find "$D/vendor.actual" -maxdepth 1 -mindepth 1 -type d | wc -l) crates"
else
	echo fail
	exit 3
fi
