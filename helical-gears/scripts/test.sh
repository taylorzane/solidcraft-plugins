#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

cli=${SOLIDCRAFT_CLI:-solidcraft-cli}
version=$(bun -p 'require("./packaging/plugin.json").version')
archive="../dist/helical-gears-$version.zip"
cargo test --locked
"$cli" plugin check "$archive"

test_dir=$(mktemp -d)
trap 'rm -rf "$test_dir"' EXIT
unzip -q "$archive" -d "$test_dir/helical-gears"
cp -R packaging/tests "$test_dir/helical-gears/tests"
"$cli" plugin test "$test_dir/helical-gears" --require-cases --out "$test_dir/designs"
SOLIDCRAFT_CLI="$cli" bun scripts/test-models.js "$test_dir"
