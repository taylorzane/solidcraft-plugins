import { basename, dirname, join, resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const { workspace } = Bun.TOML.parse(await Bun.file(join(root, "Cargo.toml")).text());
const { version } = workspace.package;
const { version: solidcraftVersion, macos_cli_sha256: cliSha256 } = workspace.metadata.solidcraft;
const versionPattern = /^\d+\.\d+\.\d+(?:-[\da-zA-Z.-]+)?(?:\+[\da-zA-Z.-]+)?$/;
const series = (value) => value.split(".").slice(0, 2).join(".");

if (!versionPattern.test(version) || !versionPattern.test(solidcraftVersion)) {
  throw new Error("Set the plugin and SolidCraft versions in the workspace Cargo.toml");
}
if (series(version) !== series(solidcraftVersion)) {
  throw new Error(`Plugin version ${version} must match the SolidCraft ${series(solidcraftVersion)} series`);
}
if (!/^[0-9a-f]{64}$/.test(cliSha256)) {
  throw new Error("Set the macOS SolidCraft CLI SHA-256 in the workspace Cargo.toml");
}

async function readManifest(path) {
  const manifest = await Bun.file(path).json();
  const directory = basename(dirname(dirname(resolve(path))));
  if (manifest.id !== directory) {
    throw new Error(`Plugin ID in ${path} must match its directory: ${directory}`);
  }
  return manifest;
}

async function pluginIds() {
  const manifests = [...new Bun.Glob("*/packaging/plugin.json").scanSync({ cwd: root })].sort();
  if (manifests.length === 0) {
    throw new Error("No plugin manifests found");
  }
  const plugins = await Promise.all(manifests.map((path) => readManifest(join(root, path))));
  return plugins.map(({ id }) => id);
}

const [action, argument] = Bun.argv.slice(2);
switch (action) {
  case "config":
    console.log(JSON.stringify({ version, solidcraft_version: solidcraftVersion, macos_cli_sha256: cliSha256 }));
    break;
  case "version":
    console.log(version);
    break;
  case "manifest":
    console.log(JSON.stringify({ ...await readManifest(argument), version }, null, 2));
    break;
  case "check-cli": {
    const actual = (await Bun.$`${argument} --version`.quiet().text()).trim();
    if (actual !== `solidcraft-cli ${solidcraftVersion}`) {
      throw new Error(`Expected solidcraft-cli ${solidcraftVersion}, got ${actual}`);
    }
    break;
  }
  case "build":
  case "test": {
    const tasks = (await pluginIds()).flatMap((id) => [`${action}:${id}`, ":::"]);
    tasks.pop();
    const run = Bun.spawn(["mise", "run", ...tasks], {
      cwd: root,
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });
    process.exitCode = await run.exited;
    break;
  }
  case "archives": {
    const archives = [];
    for (const id of await pluginIds()) {
      const path = `dist/${id}-${version}.zip`;
      const archive = join(root, path);
      if (!(await Bun.file(archive).exists())) {
        throw new Error(`Missing plugin archive ${path}`);
      }
      const manifest = await Bun.$`unzip -p ${archive} plugin.json`.quiet().json();
      if (manifest.id !== id || manifest.version !== version) {
        throw new Error(`Manifest in ${path} must have ID ${id} and version ${version}`);
      }
      archives.push(path);
    }
    console.log(archives.join("\n"));
    break;
  }
  default:
    throw new Error("Usage: bun scripts/release.js config|version|manifest|check-cli|build|test|archives");
}
