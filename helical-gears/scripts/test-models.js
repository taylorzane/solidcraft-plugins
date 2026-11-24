import assert from "node:assert/strict";
import { cp, mkdir } from "node:fs/promises";
import { join } from "node:path";

const cli = process.env.SOLIDCRAFT_CLI ?? "solidcraft-cli";
const scratch = process.argv[2];
const packagePath = join(scratch, "helical-gears");
const defaultRoot = 24 / Math.cos(Math.PI / 6) - 2.5;
const defaultTip = defaultRoot + 4.5;
const toothPitch = 2 * Math.PI / 24;

async function callTools(calls, args = []) {
  const messages = [
    {
      id: 0, method: "initialize",
      params: { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "gear-tests", version: "1" } },
    },
    { method: "notifications/initialized" },
    ...calls.map(({ name, arguments: parameters }, i) => ({
      id: i + 1, method: "tools/call", params: { name, arguments: parameters },
    })),
  ];
  const input = messages.map((message) => JSON.stringify({ jsonrpc: "2.0", ...message })).join("\n") + "\n";
  const process = Bun.spawn([cli, "mcp", "--headless", ...args], {
    stdin: new Response(input), stdout: "pipe", stderr: "pipe",
  });
  const [output, stderr, exit] = await Promise.all([
    new Response(process.stdout).text(), new Response(process.stderr).text(), process.exited,
  ]);
  assert.equal(exit, 0, stderr);
  const replies = output.trim().split("\n").map(JSON.parse);
  return calls.map((_, i) => {
    const reply = replies.find((item) => item.id === i + 1);
    assert.ok(reply?.result, JSON.stringify(reply));
    const result = reply.result;
    const text = result.content[0].text;
    return {
      error: result.isError,
      value: result.structuredContent ?? (result.isError ? text : JSON.parse(text)),
    };
  });
}

const execute = (command, params = {}) => ({ name: "execute", arguments: { command, params, verbose: false } });

function radialRay(z, angle) {
  const x = Math.cos(angle);
  const y = Math.sin(angle);
  return { origin: [80 * x, 80 * y, z], dir: [-x, -y, 0] };
}

function radius(hit) {
  assert.ok(hit, "Expected a ray to hit the gear");
  return Math.hypot(hit.point[0], hit.point[1]);
}

function near(actual, expected, tolerance, label) {
  assert.ok(Math.abs(actual - expected) < tolerance, `${label}: got ${actual}, expected ${expected}`);
}

async function checkModel({ name, toothCenters, width, tip = defaultTip, solidCenter = false, relief }) {
  const rays = [];
  const expected = [];
  const addRay = (ray, condition) => {
    rays.push(ray);
    expected.push(condition);
  };
  for (const [z, phase] of toothCenters) {
    for (let tooth = 0; tooth < 24; tooth++) {
      const angle = phase + tooth * toothPitch;
      // Sample both sides of each predicted center as well as the valley. This
      // catches tooth phase and hand errors that a bounding box would miss.
      for (const offset of [-0.014, 0, 0.014]) {
        addRay(radialRay(z, angle + offset), (hit) => near(radius(hit), tip, 0.02, `${name} tip at z=${z}`));
      }
      addRay(radialRay(z, angle + toothPitch / 2), (hit) => {
        assert.ok(radius(hit) < tip - 0.4, `${name} expected a tooth valley at z=${z}`);
      });
    }
  }
  if (relief) {
    for (const z of [relief.start + 0.05, relief.end - 0.05]) {
      for (let i = 0; i < 12; i++) {
        addRay(radialRay(z, i * Math.PI / 6), (hit) => near(radius(hit), relief.radius, 0.015, "Center relief radius"));
      }
    }
  }
  addRay({ origin: [solidCenter ? 0 : 3.98, 0, width + 1], dir: [0, 0, -1] }, (hit) => {
    if (solidCenter) near(hit?.point[2], width, 0.005, "Solid center end face");
    else assert.equal(hit, null, "The bore must be open through both faces");
  });
  addRay({ origin: [4.02, 0, width + 1], dir: [0, 0, -1] }, (hit) => {
    near(hit?.point[2], width, 0.005, "Upper end face");
  });
  addRay({ origin: [4.02, 0, -1], dir: [0, 0, 1] }, (hit) => {
    near(hit?.point[2], 0, 0.005, "Starting end face");
  });
  const [probe] = await callTools([execute("view.pick", { rays })], ["--in", join(scratch, "designs", `${name}.sc`)]);
  assert.ok(!probe.error, JSON.stringify(probe.value));
  assert.equal(probe.value.hits.length, expected.length);
  probe.value.hits.forEach((hit, index) => expected[index](hit));
  console.log(`${name}: tooth count, phase, hand, end faces, and bore verified${relief ? " along with center relief" : ""}`);
}

// With module 2, 24 teeth, and a 30° helix, the pitch-cylinder twist is
// sin(30°) / 24 radians/mm. Check both sides of each seam or relief.
const slope = 0.5 / 24;
await checkModel({ name: "helical", width: 12, toothCenters: [[0.1, slope * 0.1], [11.9, slope * 11.9]] });
await checkModel({
  name: "herringbone", width: 12,
  toothCenters: [0.1, 5.95, 6.05, 11.9].map((z) => [z, slope * Math.min(z, 12 - z)]),
});
await checkModel({
  name: "double-helical", width: 12,
  toothCenters: [0.1, 4.95, 7.05, 11.9].map((z) => [z, slope * Math.min(z, 12 - z)]),
  relief: { start: 5, end: 7, radius: defaultRoot },
});
await checkModel({
  name: "left-hand-fit", width: 8, tip: 29.5, solidCenter: true,
  toothCenters: [0.1, 7.9].map((z) => [z, Math.PI / 30 - slope * z]),
});

async function inspectPackage(path) {
  const result = await Bun.$`${cli} exec plugin.inspect ${JSON.stringify({ path })}`.quiet().json();
  return { path, approval: result.approval };
}

// Withhold mirror permission from a test-only copy to force the real host
// to fail after the component, its sketches and the first sweep have run.
const brokenPackage = join(scratch, "helical-gears-rollback");
await mkdir(brokenPackage, { recursive: true });
const manifest = await Bun.file(join(packagePath, "plugin.json")).json();
manifest.id = "helical-gears-rollback";
manifest.runtime.commands = manifest.runtime.commands.filter((command) => command !== "solid.mirror");
await Bun.write(join(brokenPackage, "plugin.json"), JSON.stringify(manifest));
await cp(join(packagePath, "helical-gears.wasm"), join(brokenPackage, "helical-gears.wasm"));
await cp(join(packagePath, "icons"), join(brokenPackage, "icons"), { recursive: true });

const installed = await inspectPackage(packagePath);
const limited = await inspectPackage(brokenPackage);
const plugin = (source, gearType, params = {}) => execute("plugin.run", {
  ...source,
  entry: "helical-gear",
  params: { gear_type: gearType, ...params },
});
const inspectComponents = execute("component.list", { compact: true, bodies: true });
const [first, second, both, rejected, afterFailure, undo, afterUndo] = await callTools([
  plugin(installed, "helical"),
  plugin(installed, "helical"),
  inspectComponents,
  plugin(limited, "herringbone"),
  inspectComponents,
  execute("edit.undo"),
  inspectComponents,
], ["--allow-path", scratch]);

for (const reply of [first, second, both, afterFailure, undo, afterUndo]) {
  assert.ok(!reply.error, JSON.stringify(reply.value));
}
assert.notEqual(first.value.result.component, second.value.result.component);
assert.notEqual(first.value.result.body, second.value.result.body);
assert.equal(first.value.result.name, "Helical Gear 1");
assert.equal(second.value.result.name, "Helical Gear 2");
assert.equal(both.value.components.length, 3, "Expected the root and two separate gear components");
for (const run of [first, second]) {
  const { component, name } = run.value.result;
  assert.equal(both.value.components.find((item) => item.id === component)?.name, name);
}
assert.ok(rejected.error, "Expected the missing command grant to fail the run");
assert.match(JSON.stringify(rejected.value), /solid\.mirror/);
assert.deepEqual(afterFailure.value, both.value, "Failed run must restore components, features, sketches, and activation");
assert.equal(afterUndo.value.components.length, 2, "One undo must remove the entire second gear");
assert.equal(afterUndo.value.components[1].id, first.value.result.component);
console.log("Repeated creation, rollback after a partial build, and one-step undo verified");

const bodyInputs = { create_as: "body", teeth: 12 };
const existingBody = "Helical Gear 1";
const bodyCommands = [
  execute("component.create", { name: "Receiver", grounded: false }),
  execute("solid.box", { corner: [10, -1, 1], length: 2, width: 2, height: 2, body_name: existingBody }),
  execute("occurrence.move", {
    occurrence: "Receiver:1",
    space: "world",
    to: { position: [100, 40, 30], axis: [0, 1, 0], angle: "90 deg" },
  }),
  execute("component.activate", { component: "Receiver", occurrence: "Receiver:1" }),
  // Plugin dimensions stay in millimeters when the design displays inches.
  execute("document.units", { units: "in" }),
  inspectComponents,
  plugin(installed, "herringbone", { create_as: "body" }),
  execute("view.pick", { rays: [
    { origin: [120, 45, 30], dir: [-1, 0, 0] },
    { origin: [90, 45, 30], dir: [1, 0, 0] },
    { origin: [120, 40, 30], dir: [-1, 0, 0] },
  ] }),
  plugin(installed, "double-helical", bodyInputs),
  plugin(installed, "helical", bodyInputs),
  inspectComponents,
  execute("inspect.measure"),
  execute("edit.undo"),
  inspectComponents,
  execute("component.activate", { component: 0 }),
  plugin(installed, "helical", { ...bodyInputs, component_name: "Root gear" }),
  inspectComponents,
];
const bodyReplies = await callTools(bodyCommands, ["--allow-path", scratch]);
bodyReplies.forEach((reply, index) => {
  assert.ok(!reply.error, `${bodyCommands[index].arguments.command}: ${JSON.stringify(reply.value)}`);
});
const [receiver, , , , , beforeBodies, chevron, picked, double, helical,
  afterBodies, measured, , undoneBody, , rootGear, inRoot] = bodyReplies.map((reply) => reply.value);
const owner = receiver.component;
const createdBodies = ["Herringbone Gear 1", "Double-Helical Gear 1", "Helical Gear 2"];
for (const [index, { result }] of [chevron, double, helical].entries()) {
  assert.equal(result.component, owner);
  assert.equal(result.name, createdBodies[index]);
  assert.equal(result.body, result.name);
}
assert.equal(afterBodies.active, owner);
assert.deepEqual(afterBodies.active_path, beforeBodies.active_path, "The selected occurrence stays active");
assert.equal(afterBodies.components.length, 2, "Adding bodies must not create new components");
const receiverBodies = (listing) => listing.components.find((component) => component.id === owner).bodies;
assert.deepEqual(receiverBodies(afterBodies), [existingBody, ...createdBodies]);
assert.deepEqual(receiverBodies(undoneBody), [existingBody, ...createdBodies.slice(0, 2)]);
const [farFace, nearFace, throughBore] = picked.hits;
assert.equal(farFace?.body, chevron.result.body);
assert.equal(nearFace?.body, chevron.result.body);
near(farFace.point[0], 112, 0.005, "Far face along the placed component's axis");
near(nearFace.point[0], 100, 0.005, "Near face along the placed component's axis");
assert.equal(throughBore, null, "The bore stays open along the placed component's axis");
for (const body of measured.bodies) {
  assert.equal(body.closed, true);
  if (body.name === existingBody) {
    near(body.volume_mm3, 8, 0.005, "Pre-existing body volume");
  } else {
    near(body.bbox.min[0], 100, 0.005, `${body.name} start along the component axis`);
    near(body.bbox.max[0], 112, 0.005, `${body.name} end along the component axis`);
  }
}
assert.equal(rootGear.result.component, 0);
assert.equal(rootGear.result.body, "Root gear");
assert.deepEqual(inRoot.components.find((component) => component.id === 0).bodies, ["Root gear"]);
assert.equal(inRoot.components.length, 2);
console.log("Separate bodies, placed component axes, root placement, naming, and undo verified");
