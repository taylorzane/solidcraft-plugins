#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

version=$(bun ../scripts/release.js version)
dist="$PWD/../dist"
archive="$dist/helical-gears-$version.zip"
target_dir=${CARGO_TARGET_DIR:-target}

mkdir -p "$dist"
staging=$(mktemp -d "$dist/.helical-gears.XXXXXX")
trap 'rm -rf "$staging"' EXIT
package="$staging/helical-gears"
mkdir "$package"

cargo build --locked --release --target wasm32-unknown-unknown --target-dir "$target_dir"
"${WASM_TOOLS:-wasm-tools}" component new \
    "$target_dir/wasm32-unknown-unknown/release/solidcraft_helical_gears.wasm" \
    -o "$package/helical-gears.wasm"
bun ../scripts/release.js manifest packaging/plugin.json > "$package/plugin.json"
cp ../LICENSE "$package/"
bun scripts/licenses.js "$package/THIRD-PARTY-LICENSES.txt"
rust_sysroot=$("${RUSTC:-rustc}" --print sysroot)
cp "$rust_sysroot/share/doc/rust/COPYRIGHT-library.html" "$package/RUST-STDLIB-LICENSES.html"
mkdir "$package/icons"
cp packaging/icons/helical-gear.svg "$package/icons/"

(cd "$package" && zip -q "$staging/helical-gears.zip" \
    plugin.json helical-gears.wasm LICENSE THIRD-PARTY-LICENSES.txt \
    RUST-STDLIB-LICENSES.html icons/helical-gear.svg)
mv "$staging/helical-gears.zip" "$archive"
printf '%s\n' "$(cd "$dist" && pwd)/helical-gears-$version.zip"
