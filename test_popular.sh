#!/bin/sh

set -e

N=30

POP=$( \
	curl -s "https://crates.io/api/v1/crates?page=50&per_page=$N&sort=downloads" \
		| jq -r '.crates[] | "\(.name)@=\(.default_version)"' \
)

for crate in $POP; do
	echo "$crate: $(./test_on_crate.sh "$crate" | tr '\n' ' ')"
done
