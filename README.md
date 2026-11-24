# SolidCraft plugins

| Plugin | Tested SolidCraft version |
| --- | --- |
| [Helical Gears](helical-gears/) | 0.47.0 |

## Build and test

Install [mise](https://mise.jdx.dev/) and run these commands from the repository root.

```sh
mise install
mise run build:helical-gears
mise run test:helical-gears
mise run build
mise run test
```

`build` and `test` run for every plugin. Each test task builds its plugin first. Builds produce installable ZIPs in `dist/`.

Tests use `solidcraft-cli` from your `PATH` by default. On macOS, you can point to the CLI bundled with SolidCraft.

```sh
SOLIDCRAFT_CLI=/Applications/SolidCraft.app/Contents/MacOS/solidcraft-cli mise run test
```

## Versioning

The major and minor numbers match the SolidCraft series we test against. The patch number tracks plugin updates within that series. Helical Gears `v0.47.1` starts the `0.47` series and was tested with SolidCraft `0.47.0`.

## Publish a plugin

GitHub Actions builds and tests every plugin on pull requests, pushes to `main`, and manual runs. Download the installable ZIPs from the run's **plugin-zips** artifact.

To publish one plugin, push a tag in the form `<plugin-id>-v<version>`, where `<version>` matches `packaging/plugin.json`. For Helical Gears 0.47.1, tag the commit you want to release and push the tag.

```sh
git tag -a helical-gears-v0.47.1 -m "Helical Gears 0.47.1"
git push origin helical-gears-v0.47.1
```

The tag runs that plugin's build and tests. After they pass, the workflow uploads `helical-gears-0.47.1.zip` to the corresponding [GitHub Release](https://github.com/taylorzane/solidcraft-plugins/releases). Install the ZIP through **Preferences → Plugins → Install plugin…**.

## Add a plugin

Create a top-level directory named for the plugin ID. Put its manifest in `packaging/plugin.json` and define `build:<id>` and `test:<id>` in `tasks.toml`. Set the tasks' `dir` to the plugin directory and make the test task depend on the build task. The root mise config discovers those task files automatically. Write the package to `dist/<plugin-id>-<version>.zip` with `plugin.json` at the root of the ZIP.

## License

Original plugin code is available under [BSD-3-Clause](LICENSE). Each plugin lists its third-party terms alongside its source and includes those notices in its installable ZIP. Users can choose how to license designs they create with the plugins, subject to any rights in material they incorporate.
