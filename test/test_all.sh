#!/usr/bin/env bash

set -e

DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

# some real-life rust programs from their repositories
echo "testing fixture 0 (bootstrap)" >&2
"$DIR"/test_sets.sh fetch-repo <<<"$DIR/.."

# more popular crates, with fewer dependencies, small test
echo "testing fixture 1 (fetch+vendor, synthetic crates)" >&2
"$DIR"/test_sets.sh fetch vendor <"$DIR"/sets_fixture_3_10_1_20250727

# less popular crates, with more dependencies, large test
echo "testing fixture 2 (fetch+vendor, synthetic crates)" >&2
"$DIR"/test_sets.sh fetch vendor <"$DIR"/sets_fixture_3_150_10_20250727

# some real-life rust programs from their repositories
echo "testing fixture 3 (fetch, real crates)" >&2
"$DIR"/test_sets.sh fetch-repo <"$DIR"/repos_fixture
