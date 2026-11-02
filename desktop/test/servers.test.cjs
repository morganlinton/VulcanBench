"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const http = require("node:http");
const os = require("node:os");
const path = require("node:path");

const servers = require("../lib/servers.cjs");

test("augmentedPath puts existing extras first, once, and keeps the rest", () => {
  const exists = (d) => d === "/opt/homebrew/bin" || d === "/usr/local/bin";
  const out = servers.augmentedPath(
    "/usr/bin:/bin:/usr/local/bin",
    ["/opt/homebrew/bin", "/usr/local/bin", "/nope", "/opt/homebrew/bin"],
    exists,
  );
  assert.equal(out, "/opt/homebrew/bin:/usr/bin:/bin:/usr/local/bin");
});

test("augmentedPath works with no PATH at all", () => {
  const out = servers.augmentedPath(undefined, ["/x"], () => true);
  assert.equal(out, "/x:/usr/bin:/bin:/usr/sbin:/sbin");
});

test("resolveRepoRoot prefers the env var, then build-info, then the dev root", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "vb-desktop-"));
  const info = path.join(dir, "build-info.json");
  assert.deepEqual(
    servers.resolveRepoRoot({ env: { VULCANBENCH_REPO: "/env/repo" }, buildInfoPath: info, devRoot: "/dev" }),
    { root: "/env/repo", source: "VULCANBENCH_REPO" },
  );
  assert.deepEqual(
    servers.resolveRepoRoot({ env: {}, buildInfoPath: info, devRoot: "/dev" }),
    { root: "/dev", source: "desktop/.." },
  );
  fs.writeFileSync(info, JSON.stringify({ repoRoot: "/built/repo" }));
  assert.deepEqual(
    servers.resolveRepoRoot({ env: {}, buildInfoPath: info, devRoot: "/dev" }),
    { root: "/built/repo", source: "build-info.json" },
  );
  fs.writeFileSync(info, "not json");
  assert.equal(servers.resolveRepoRoot({ env: {}, buildInfoPath: info, devRoot: "/dev" }).root, "/dev");
});

test("repoProblems names what a checkout is missing", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "vb-desktop-"));
  assert.deepEqual(servers.repoProblems(path.join(dir, "nope")), [
    `checkout not found at ${path.join(dir, "nope")}`,
  ]);
  const problems = servers.repoProblems(dir);
  assert.equal(problems.length, 3);
  assert.ok(problems.every((p) => p.startsWith("missing ")));
  fs.mkdirSync(path.join(dir, "scripts"));
  fs.mkdirSync(path.join(dir, ".venv", "bin"), { recursive: true });
  fs.writeFileSync(path.join(dir, "scripts", "gui_backend.sh"), "");
  fs.writeFileSync(path.join(dir, ".venv", "bin", "python"), "");
  fs.writeFileSync(path.join(dir, "vulcanbench.toml"), "");
  assert.deepEqual(servers.repoProblems(dir), []);
});

test("waitForHttp resolves once a local server answers and gives up when told the process died", async () => {
  const srv = http.createServer((_req, res) => res.end("ok"));
  await new Promise((r) => srv.listen(0, "127.0.0.1", r));
  const { port } = srv.address();
  const url = `http://127.0.0.1:${port}/`;
  assert.equal(await servers.waitForHttp(url, { timeoutMs: 2000 }), true);
  srv.close();
  await new Promise((r) => srv.once("close", r));
  assert.equal(await servers.waitForHttp(url, { timeoutMs: 2000, alive: () => false }), false);
});

test("pickPort returns the preferred port when free and another one when taken", async () => {
  const hold = http.createServer();
  await new Promise((r) => hold.listen(0, "127.0.0.1", r));
  const taken = hold.address().port;
  const chosen = await servers.pickPort(taken);
  assert.notEqual(chosen, taken);
  assert.ok(chosen > 0);
  hold.close();
  await new Promise((r) => hold.once("close", r));
  assert.equal(await servers.pickPort(taken), taken);
});

test("startDashboard refuses when the build is missing", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "vb-desktop-"));
  assert.throws(
    () => servers.startDashboard({ dashboardDir: dir, port: 1, logDir: dir }),
    /dashboard build missing/,
  );
});

test("startDashboard runs server.js with the app's node and the chosen port", async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "vb-desktop-"));
  fs.writeFileSync(
    path.join(dir, "server.js"),
    "process.stdout.write(JSON.stringify({port: process.env.PORT, host: process.env.HOSTNAME, asNode: process.env.ELECTRON_RUN_AS_NODE}));",
  );
  const { child, log } = servers.startDashboard({ dashboardDir: dir, port: 4321, execPath: process.execPath, logDir: dir });
  await new Promise((r) => child.once("exit", r));
  const out = fs.readFileSync(log, "utf8");
  assert.match(out, /"port":"4321"/);
  assert.match(out, /"host":"127\.0\.0\.1"/);
  assert.match(out, /"asNode":"1"/);
});

test("stopChild ends a child and resolves for one that already exited", async () => {
  const { spawn } = require("node:child_process");
  const sleeper = spawn("sleep", ["30"]);
  await servers.stopChild(sleeper, { graceMs: 1000 });
  assert.notEqual(sleeper.exitCode === null && sleeper.signalCode === null, true);
  await servers.stopChild(sleeper);
  await servers.stopChild(null);
});
