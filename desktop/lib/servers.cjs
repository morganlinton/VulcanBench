// Helpers the Electron main process uses to find the checkout, start the two
// servers and wait for them. Kept free of Electron imports so `node --test`
// can exercise them.
"use strict";

const fs = require("node:fs");
const http = require("node:http");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");

// The dashboard bakes the backend address in at build time
// (NEXT_PUBLIC_API_BASE, default http://localhost:8000), so the backend port
// is fixed. The dashboard's own port is chosen at runtime.
const BACKEND_PORT = 8000;
const DASHBOARD_PORT_PREFERRED = 3000;

// Directories a Finder-launched app does not have on PATH but the backend's
// subprocesses need (docker, uv, node, the agent CLIs). Only existing ones are
// added, each once, ahead of whatever PATH already holds.
function pathExtras(home = os.homedir()) {
  return [
    path.join(home, ".local", "bin"),
    path.join(home, ".local", "node", "bin"),
    path.join(home, ".docker", "bin"),
    "/opt/homebrew/bin",
    "/opt/homebrew/opt/node@22/bin",
    "/usr/local/bin",
  ];
}

function augmentedPath(current, extras = pathExtras(), exists = fs.existsSync) {
  const parts = (current || "/usr/bin:/bin:/usr/sbin:/sbin").split(":").filter(Boolean);
  const front = [];
  for (const dir of extras) {
    if (exists(dir) && !parts.includes(dir) && !front.includes(dir)) front.push(dir);
  }
  return [...front, ...parts].join(":");
}

// Where the VulcanBench checkout is. The app is a launcher for a checkout, not
// a self-contained install: the Python venv, scripts and run worktrees stay
// where they are. Order: VULCANBENCH_REPO, the build-info file the packaging
// step writes, then the repo this desktop/ folder sits in (development).
function resolveRepoRoot({ env = process.env, buildInfoPath, devRoot } = {}) {
  if (env.VULCANBENCH_REPO) return { root: env.VULCANBENCH_REPO, source: "VULCANBENCH_REPO" };
  if (buildInfoPath && fs.existsSync(buildInfoPath)) {
    try {
      const info = JSON.parse(fs.readFileSync(buildInfoPath, "utf8"));
      if (info.repoRoot) return { root: info.repoRoot, source: "build-info.json" };
    } catch {
      // fall through to the development root
    }
  }
  return { root: devRoot, source: "desktop/.." };
}

// What a usable checkout must have before anything is started.
function repoProblems(root) {
  const problems = [];
  if (!root || !fs.existsSync(root)) return [`checkout not found at ${root}`];
  for (const rel of ["scripts/gui_backend.sh", ".venv/bin/python", "vulcanbench.toml"]) {
    if (!fs.existsSync(path.join(root, rel))) problems.push(`missing ${rel} under ${root}`);
  }
  return problems;
}

function httpOk(url, timeoutMs = 1500) {
  return new Promise((resolve) => {
    const req = http.get(url, { timeout: timeoutMs }, (res) => {
      res.resume();
      resolve(res.statusCode !== undefined && res.statusCode < 500);
    });
    req.on("timeout", () => {
      req.destroy();
      resolve(false);
    });
    req.on("error", () => resolve(false));
  });
}

async function waitForHttp(url, { timeoutMs = 60000, intervalMs = 400, alive = () => true } = {}) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await httpOk(url)) return true;
    if (!alive()) return false;
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  return false;
}

function portFree(port, host = "127.0.0.1") {
  return new Promise((resolve) => {
    const srv = net.createServer();
    srv.once("error", () => resolve(false));
    srv.listen(port, host, () => srv.close(() => resolve(true)));
  });
}

async function pickPort(preferred = DASHBOARD_PORT_PREFERRED, host = "127.0.0.1") {
  if (await portFree(preferred, host)) return preferred;
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.once("error", reject);
    srv.listen(0, host, () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

function openLog(dir, name) {
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, name);
  const fd = fs.openSync(file, "a");
  fs.writeSync(fd, `\n=== ${new Date().toISOString()} start\n`);
  return { file, fd };
}

// The backend is the existing scripts/gui_backend.sh: uvicorn from the
// checkout's venv, runner enabled, loopback only. Nothing here reads
// credentials; the script's process inherits the user's environment plus the
// PATH additions, exactly as a terminal launch would.
function startBackend({ repoRoot, logDir, env = process.env, port = BACKEND_PORT }) {
  const log = openLog(logDir, "backend.log");
  const child = spawn("bash", ["scripts/gui_backend.sh", String(port)], {
    cwd: repoRoot,
    env: { ...env, PATH: augmentedPath(env.PATH) },
    stdio: ["ignore", log.fd, log.fd],
    detached: false,
  });
  return { child, log: log.file };
}

// The dashboard is Next's standalone server, run with the app's own node
// (ELECTRON_RUN_AS_NODE) so no node install is needed on the machine.
function startDashboard({ dashboardDir, port, execPath = process.execPath, logDir, env = process.env }) {
  const server = path.join(dashboardDir, "server.js");
  if (!fs.existsSync(server)) throw new Error(`dashboard build missing: ${server}`);
  const log = openLog(logDir, "dashboard.log");
  const child = spawn(execPath, [server], {
    cwd: dashboardDir,
    env: {
      ...env,
      ELECTRON_RUN_AS_NODE: "1",
      NODE_ENV: "production",
      NEXT_TELEMETRY_DISABLED: "1",
      HOSTNAME: "127.0.0.1",
      PORT: String(port),
    },
    stdio: ["ignore", log.fd, log.fd],
    detached: false,
  });
  return { child, log: log.file };
}

function stopChild(child, { graceMs = 4000 } = {}) {
  return new Promise((resolve) => {
    if (!child || child.exitCode !== null || child.signalCode !== null) return resolve();
    const timer = setTimeout(() => {
      try {
        child.kill("SIGKILL");
      } catch {
        // already gone
      }
    }, graceMs);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
    try {
      child.kill("SIGTERM");
    } catch {
      clearTimeout(timer);
      resolve();
    }
  });
}

module.exports = {
  BACKEND_PORT,
  DASHBOARD_PORT_PREFERRED,
  augmentedPath,
  httpOk,
  openLog,
  pathExtras,
  pickPort,
  portFree,
  repoProblems,
  resolveRepoRoot,
  startBackend,
  startDashboard,
  stopChild,
  waitForHttp,
};
