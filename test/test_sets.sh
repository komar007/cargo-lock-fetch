#!/usr/bin/env bash

set -e

DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

FETCH=0
VENDOR=0
FETCH_REPO=0
while [ $# -gt 0 ]; do
	case "$1" in
		fetch) FETCH=1 ;;
		vendor) VENDOR=1 ;;
		fetch-repo) FETCH_REPO=1 ;;
		*) exit 1 ;;
	esac
	shift
done

i=1
while read -r test; do
	if [ $FETCH -eq 1 ]; then
		echo -n "test $i/fetch: "
		# shellcheck disable=SC2086
		"$DIR"/test_fetch.sh $test
	fi
	if [ $VENDOR -eq 1 ]; then
		echo -n "test $i/vendor: "
		# shellcheck disable=SC2086
		"$DIR"/test_vendor.sh $test
	fi
	if [ $FETCH_REPO -eq 1 ]; then
		echo -n "test $i/fetch-repo: "
		# shellcheck disable=SC2086
		"$DIR"/test_fetch_repo.sh $test
	fi
	_=$((i++))
done
