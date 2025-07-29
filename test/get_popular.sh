#!/usr/bin/env bash

set -e

CRATES_IO="https://crates.io/api/v1/crates"
MAX_CRATES_IO=100

N=${1:-3}
PER_TEST=${2:-10}
START=${3:-0}

TOTAL=$((N * PER_TEST))
NUM_REQUESTS=$(((TOTAL + MAX_CRATES_IO - 1) / MAX_CRATES_IO))

if [ "$NUM_REQUESTS" -eq 1 ]; then
	PER_REQ=$TOTAL
else
	PER_REQ=$MAX_CRATES_IO
fi

echo -n "downloading popular crates list" >&2
for i in $(seq $NUM_REQUESTS); do
	POP+=$(
		curl -s "$CRATES_IO?page=$((i + START))&per_page=$PER_REQ&sort=downloads" |
			jq -r '.crates[] | "\(.name)@=\(.default_version)"' |
			tr '\n' ' '
	)
	echo -n . >&2
done
POP=$(tr ' ' '\n' <<<"$POP")
echo " done" >&2

for i in $(seq "$N"); do
	crates=$(sed -n "$(seq "$i" "$N" $TOTAL | tr '\n' ' ' | sed 's/ /p;/g')" <<<"$POP")
	# shellcheck disable=SC2086
	echo $crates
done
