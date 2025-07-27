#!/usr/bin/env bash

set -e

DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

# more popular crates, with fewer dependencies, small test
echo "testing fixture 1" >&2
"$DIR"/test_sets.sh < "$DIR"/sets_fixture_3_10_1_20250727

# less popular crates, with more dependencies, large test
echo "testing fixture 2" >&2
"$DIR"/test_sets.sh < "$DIR"/sets_fixture_3_150_10_20250727
