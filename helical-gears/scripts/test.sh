#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

cli=${SOLIDCRAFT_CLI:-solidcraft-cli}
version=$(bun ../scripts/release.js version)
archive="../dist/helical-gears-$version.zip"
bun ../scripts/release.js check-cli "$cli"
cargo test --locked
"$cli" plugin check "$archive"

test_dir=$(mktemp -d)
trap 'rm -rf "$test_dir"' EXIT
unzip -q "$archive" -d "$test_dir/helical-gears"
for notice in THIRD-PARTY-LICENSES.txt RUST-STDLIB-LICENSES.html; do
    if [ ! -s "$test_dir/helical-gears/$notice" ]; then
        printf 'Missing or empty %s in %s\n' "$notice" "$archive" >&2
        exit 1
    fi
done
cp -R packaging/tests "$test_dir/helical-gears/tests"
"$cli" plugin test "$test_dir/helical-gears" --require-cases --out "$test_dir/designs"
SOLIDCRAFT_CLI="$cli" bun scripts/test-models.js "$test_dir"
