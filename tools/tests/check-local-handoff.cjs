/* Execute actual registry-exported URLs through the production host parser. */
"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const launch = require("../../apps/game-client/web/launch-options.js");
const urls = fs.readFileSync(0, "utf8").trim().split("\n").filter(Boolean);
assert.equal(urls.length, 18, "exporter must execute both locales and every clock corner");
const controls = new Set();
const locales = new Set();
const configurations = new Set();
for (const raw of urls) {
  const url = new URL(raw, "https://example.invalid");
  assert.equal(url.pathname, "/play/local/");
  const config = launch.parse(url.search);
  assert.equal(config.source, "tabula");
  assert.equal(config.returnTo, "/games/com.tabula.chess?setup=1");
  controls.add(config.clock);
  locales.add(config.locale);
  configurations.add([config.clock, config.locale, config.initialMs, config.clock === "bronstein" ? config.delayMs : config.incrementMs].join(":"));
  assert.deepEqual(launch.parse("?" + launch.query(config)), config);
  const resolved = launch.resolve(config, () => ({matches:false}));
  const args = launch.argumentsFor(resolved).split("\n");
  assert.equal(args[args.indexOf("--clock") + 1], url.searchParams.get("clock"));
  assert(args.includes("--skip-setup"));
  assert(!args.some((argument) => argument.includes("/games/") || argument.includes("return_to") || argument === "tabula"));
  if (config.clock === "untimed") {
    assert(!args.includes("--initial-ms"));
  } else {
    assert.equal(args[args.indexOf("--initial-ms") + 1], url.searchParams.get("initial_ms"));
    const adjustment = config.clock === "fischer" ? "increment" : "delay";
    assert.equal(args[args.indexOf("--" + adjustment + "-ms") + 1], url.searchParams.get(adjustment + "_ms"));
  }
}
assert.equal(configurations.size, 18, "duplicate exports must not replace a boundary case");
assert.deepEqual([...controls].sort(), ["bronstein", "fischer", "untimed"]);
assert.deepEqual([...locales].sort(), ["en", "vi"]);
console.log("PASS: 18 actual registry URLs → host config → Rust launch arguments");
