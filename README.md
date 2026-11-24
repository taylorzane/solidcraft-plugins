# SolidCraft plugins

- [Helical Gears](helical-gears/)

## Build and test

Install [mise](https://mise.jdx.dev/) and run these commands from the repository root.

```sh
mise install
mise run build:helical-gears
mise run test:helical-gears
```

`mise run package:helical-gears` is an alias for building that plugin's installable ZIP in `dist/`. Each plugin's test task builds and tests its own ZIP. Use `mise run build` or `mise run test` to run across every plugin.

Tests require the SolidCraft CLI version pinned in [Cargo.toml](Cargo.toml) and use `solidcraft-cli` from your `PATH` by default. On macOS, you can point to the CLI bundled with the matching SolidCraft installation.

```sh
SOLIDCRAFT_CLI=/Applications/SolidCraft.app/Contents/MacOS/solidcraft-cli mise run test:helical-gears
```

## Versioning

All plugins use `workspace.package.version` from [Cargo.toml](Cargo.toml) for their Rust crates, packaged manifests, and ZIP filenames. The major and minor numbers match the SolidCraft series we test against. The patch number counts releases of this repository within that series. The first release, `v0.47.1`, targets SolidCraft `0.47.0`.

To change the SolidCraft target, update the version and macOS CLI SHA-256 under `workspace.metadata.solidcraft` in [Cargo.toml](Cargo.toml) and refresh [the shared WIT interface](wit/plugin.wit) from `crates/craft-plugin/wit/plugin.wit` at the corresponding upstream tag. CI verifies that the WIT file and CLI match the pin.

## Publish all plugins

GitHub Actions builds and tests every plugin on pull requests, pushes to `main`, and manual runs. Download the installable ZIPs from the run's **plugin-zips** artifact.

Set the shared plugin version in `workspace.package.version`, run `mise exec -- cargo update --workspace` to refresh the lockfile, and commit the release changes. To publish, push one tag that matches the shared version, for example `v0.47.1`.

```sh
git tag -a v0.47.1 -m "Release 0.47.1"
git push origin v0.47.1
```

The tag builds and tests every plugin. After all tests pass, the workflow publishes one ZIP per plugin to the same [GitHub Release](https://github.com/taylorzane/solidcraft-plugins/releases). Install the ZIP for the plugin you want through **Preferences → Plugins → Install plugin…**.

## Add a plugin

Create a top-level directory named for the plugin ID and put the manifest template in `packaging/plugin.json`. Define `build:<id>` and `test:<id>` in `tasks.toml`. Set the tasks' `dir` to the plugin directory, make the test task depend on the build task, and give the build task a `package:<id>` alias. The root mise tasks discover plugins by their manifest.

Add Rust crates to `workspace.members`, inherit the version with `version.workspace = true`, and use the shared `wit/` directory for Wasm bindings. In the build script, use `bun ../scripts/release.js version` for the ZIP filename and `bun ../scripts/release.js manifest packaging/plugin.json` to write the versioned `plugin.json` at the root of the ZIP. Save the ZIP as `dist/<plugin-id>-<version>.zip` in the repository root. In the test script, use `bun ../scripts/release.js check-cli "$cli"` before running tests with SolidCraft.

## License

Original plugin code is available under [BSD-3-Clause](LICENSE). Each plugin lists its third-party terms alongside its source and includes those notices in its installable ZIP. Users can choose how to license designs they create with the plugins, subject to any rights in material they incorporate.

Run `mise run licenses` to refresh the checked-in notices. Plugin builds generate their own copy from the locked Wasm dependencies and include Rust's standard library notices.
