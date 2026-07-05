#!/bin/sh

# Verify that lockfiles pinning yanked versions still fetch and vendor.
#
# Case "partial": crossbeam-channel 0.5.13 is yanked but other 0.5.x
# versions exist (the exact crate from issue #26); built with cargo and
# pinned via `cargo update --precise`, which permits yanked versions.
# Case "allyanked": every version of core2 is yanked, so the reference
# project cannot be built with cargo at all and is written by hand with
# checksums recorded from crates.io (immutable once published).

set -e

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/..

cd "$ROOT"

cargo -q build
TARGET=$(realpath target/debug/cargo-lock-fetch)

D=$(mktemp -d)
cleanup() {
	rm -rf "$D"
}
trap cleanup EXIT

mkdir "$D/partial"
cd "$D/partial"
cargo -q init . --vcs none --name package
cargo -q add crossbeam-channel@0.5
cargo -q update crossbeam-channel --precise 0.5.13
cd - >/dev/null

mkdir "$D/allyanked"
cd "$D/allyanked"
cargo -q init . --vcs none --name package
echo 'core2 = { version = "=0.4.0", default-features = false }' >>Cargo.toml
cat >Cargo.lock <<'EOF'
version = 4

[[package]]
name = "core2"
version = "0.4.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "b49ba7ef1ad6107f8824dbe97de947cbaac53c44e7f9756a1fba0d37c1eec505"
dependencies = [
 "memchr",
]

[[package]]
name = "memchr"
version = "2.8.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "88904434abc2901f197fe8cc55f0445e7ded921dba5911dad2e2b39b48e663c4"

[[package]]
name = "package"
version = "0.1.0"
dependencies = [
 "core2",
]
EOF
cd - >/dev/null

check() {
	case=$1
	crate=$2

	CARGO_HOME="$D/cargo_home_$case"
	export CARGO_HOME
	"$TARGET" lock-fetch -q --lockfile-path "$D/$case/Cargo.lock"
	find "$CARGO_HOME/registry/cache" -name "$crate.crate" | grep -q . || {
		echo "fail: $crate not fetched for $case"
		exit 1
	}
	(cd "$D/$case" && cargo -q fetch --frozen) || {
		echo "fail: cargo fetch --frozen failed for $case"
		exit 1
	}
	echo "pass: fetched $case ($crate)"

	(cd "$D/$case" && cargo -q vendor "$D/vendor_expected_$case" --versioned-dirs >/dev/null) || {
		echo "fail: reference cargo vendor failed for $case"
		exit 1
	}
	"$TARGET" lock-fetch -q --lockfile-path "$D/$case/Cargo.lock" \
		--vendor "$D/vendor_actual_$case" -- --versioned-dirs
	if ! diff -Naur "$D/vendor_expected_$case" "$D/vendor_actual_$case" >/dev/null; then
		echo "fail: vendor mismatch for $case"
		exit 1
	fi
	echo "pass: vendored $case ($crate)"
	unset CARGO_HOME
}

check partial crossbeam-channel-0.5.13
check allyanked core2-0.4.0
