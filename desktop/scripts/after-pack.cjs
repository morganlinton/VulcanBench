// electron-builder afterPack hook: copy the assembled dashboard (Next's
// standalone server, its node_modules and .next) and build-info.json into the
// app's Resources folder. electron-builder's extraResources applies its own
// file filters and drops node_modules, which the dashboard server needs.
"use strict";

const fs = require("node:fs");
const path = require("node:path");

module.exports = async function afterPack(context) {
  const here = path.resolve(__dirname, "..");
  const src = path.join(here, "resources", "dashboard");
  const info = path.join(here, "resources", "build-info.json");
  if (!fs.existsSync(path.join(src, "server.js"))) {
    throw new Error(`dashboard build missing at ${src}; run scripts/build-dashboard.sh first`);
  }
  const appName = `${context.packager.appInfo.productFilename}.app`;
  const resources = path.join(context.appOutDir, appName, "Contents", "Resources");
  const dest = path.join(resources, "dashboard");
  fs.rmSync(dest, { recursive: true, force: true });
  fs.cpSync(src, dest, { recursive: true, dereference: true });
  fs.copyFileSync(info, path.join(resources, "build-info.json"));
  const entries = fs.readdirSync(path.join(dest, "node_modules")).length;
  console.log(`  • dashboard copied to ${dest} (${entries} node_modules entries)`);
};
