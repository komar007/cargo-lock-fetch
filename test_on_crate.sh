#!/bin/sh

set -e

if [ -z "$1" ]; then
	echo "usage: $0 crate"
	exit 1
fi

D=$(mktemp -d) || exit
cleanup() {
    rm -rf "$D"
}
trap cleanup EXIT
mkdir "$D/fake"

cd "$D/fake"
cargo -q init . --vcs none --name compare
cargo -q add "$1"
t0=$(date +%s%N)
cargo -q vendor "../vendor.expected/" --versioned-dirs
t1=$(date +%s%N)
t_cargo=$((t1-t0))
mv Cargo.lock ../
cd - > /dev/null || exit 2
rm -fr "$D/fake"

t0=$(date +%s%N)
cargo -q run -- lock-fetch -q --lockfile-path "$D/Cargo.lock" --vendor "$D/vendor.actual/" --versioned-dirs
t1=$(date +%s%N)
t_fetch=$((t1-t0))

if diff -Naur "$D/vendor.expected/" "$D/vendor.actual/"; then
	echo success
else
	echo fail
	exit 3
fi

echo "vendored $(find "$D/vendor.actual" -maxdepth 1 -type d | wc -l) crates"

echo t_cargo=$((t_cargo/1000000))ms t_fetch=$((t_fetch/1000000))ms
