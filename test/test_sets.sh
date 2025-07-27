#!/usr/bin/env bash

set -e

DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

i=1
while read -r crates; do
	echo -n "test $i/fetch: "
	# shellcheck disable=SC2086
	"$DIR"/test_fetch.sh $crates
	echo -n "test $i/vendor: "
	# shellcheck disable=SC2086
	"$DIR"/test_vendor.sh $crates
	_=$((i++))
done
