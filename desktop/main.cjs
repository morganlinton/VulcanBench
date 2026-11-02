// VulcanBench Mac app: a window on the local run GUI.
//
// On launch it starts the backend (scripts/gui_backend.sh from the checkout,
// unless one already answers on its port) and the dashboard (Next's
// standalone server, run with the app's own node), waits for both, then shows
// the Runs page. Quitting stops the servers it started. It never signals a
// benchmark run: the backend's supervisor owns those and leaves them running
// across restarts, exactly as with a terminal launch.
"use strict";

const path = require("node:path");
const fs = require("node:fs");
const os = require("node:os");
const { app, BrowserWindow, Menu, dialog, shell, ipcMain } = require("electron");
const servers = require("./lib/servers.cjs");

const LOG_DIR = path.join(os.homedir(), "Library", "Logs", "VulcanBench");
const APP_PAGE = "/runs";

const state = {
  win: null,
  backend: null, // { child, log } when this app started it
  dashboard: null,
  dashboardPort: null,
  backendReused: false,
  repoRoot: null,
  status: "starting",
};

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on("second-instance", () => {
    if (state.win) {
      if (state.win.isMinimized()) state.win.restore();
      state.win.focus();
    }
  });
  app.whenReady().then(main).catch((err) => {
    dialog.showErrorBox("VulcanBench could not start", String(err && err.stack ? err.stack : err));
    app.quit();
  });
}

function setStatus(text) {
  state.status = text;
  if (state.win && !state.win.isDestroyed()) state.win.webContents.send("status", text);
}

function dashboardDir() {
  return app.isPackaged
    ? path.join(process.resourcesPath, "dashboard")
    : path.join(__dirname, "resources", "dashboard");
}

function buildInfoPath() {
  return app.isPackaged
    ? path.join(process.resourcesPath, "build-info.json")
    : path.join(__dirname, "resources", "build-info.json");
}

function createWindow() {
  const win = new BrowserWindow({
    width: 1360,
    height: 920,
    minWidth: 900,
    minHeight: 600,
    title: "VulcanBench",
    show: false,
    backgroundColor: "#0b0b0b",
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  win.once("ready-to-show", () => win.show());
  // Links that leave the local GUI open in the default browser.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (isLocal(url)) return { action: "allow" };
    shell.openExternal(url);
    return { action: "deny" };
  });
  win.webContents.on("will-navigate", (event, url) => {
    if (!isLocal(url) && !url.startsWith("file:")) {
      event.preventDefault();
      shell.openExternal(url);
    }
  });
  win.on("closed", () => {
    state.win = null;
  });
  return win;
}

function isLocal(url) {
  try {
    const u = new URL(url);
    return (u.hostname === "127.0.0.1" || u.hostname === "localhost") &&
      (u.port === String(state.dashboardPort) || u.port === String(servers.BACKEND_PORT));
  } catch {
    return false;
  }
}

function dashboardUrl(p = APP_PAGE) {
  return `http://127.0.0.1:${state.dashboardPort}${p}`;
}

function buildMenu() {
  const template = [
    {
      label: app.name,
      submenu: [
        { role: "about" },
        { type: "separator" },
        {
          label: "Open in Browser",
          accelerator: "CmdOrCtrl+Shift+O",
          click: () => state.dashboardPort && shell.openExternal(dashboardUrl()),
        },
        {
          label: "Show Logs in Finder",
          click: () => shell.openPath(LOG_DIR),
        },
        {
          label: "Show Checkout in Finder",
          click: () => state.repoRoot && shell.openPath(state.repoRoot),
        },
        { type: "separator" },
        { role: "hide" },
        { role: "hideOthers" },
        { role: "unhide" },
        { type: "separator" },
        { role: "quit" },
      ],
    },
    { role: "editMenu" },
    {
      label: "View",
      submenu: [
        {
          label: "Runs",
          accelerator: "CmdOrCtrl+1",
          click: () => state.win && state.win.loadURL(dashboardUrl("/runs")),
        },
        {
          label: "Launch",
          accelerator: "CmdOrCtrl+2",
          click: () => state.win && state.win.loadURL(dashboardUrl("/launch")),
        },
        {
          label: "Leaderboard",
          accelerator: "CmdOrCtrl+3",
          click: () => state.win && state.win.loadURL(dashboardUrl("/leaderboard")),
        },
        { type: "separator" },
        { role: "reload" },
        { role: "toggleDevTools" },
        { type: "separator" },
        { role: "resetZoom" },
        { role: "zoomIn" },
        { role: "zoomOut" },
        { type: "separator" },
        { role: "togglefullscreen" },
      ],
    },
    { role: "windowMenu" },
  ];
  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

async function main() {
  app.setName("VulcanBench");
  if (!app.isPackaged && process.platform === "darwin") {
    const png = path.join(__dirname, "build", "icon.png");
    if (fs.existsSync(png)) app.dock.setIcon(png);
  }
  buildMenu();
  ipcMain.handle("status", () => state.status);

  state.win = createWindow();
  await state.win.loadFile(path.join(__dirname, "splash.html"));

  const { root, source } = servers.resolveRepoRoot({
    buildInfoPath: buildInfoPath(),
    devRoot: path.resolve(__dirname, ".."),
  });
  state.repoRoot = root;
  const problems = servers.repoProblems(root);
  if (problems.length) {
    await fail(
      "This app launches the VulcanBench checkout it was built from, and that checkout is not usable.",
      [
        ...problems,
        `checkout path came from ${source}`,
        "Run `make setup` in the checkout, or set VULCANBENCH_REPO to another checkout and relaunch.",
      ].join("\n"),
    );
    return;
  }

  // Backend: reuse one already on the port (make gui, or a previous copy of
  // the app that is still shutting down), otherwise start it.
  const health = `http://127.0.0.1:${servers.BACKEND_PORT}/api/health`;
  if (await servers.httpOk(health)) {
    state.backendReused = true;
    setStatus("Backend already running on port 8000, using it");
  } else {
    setStatus("Starting the backend");
    state.backend = servers.startBackend({ repoRoot: root, logDir: LOG_DIR });
    const ok = await servers.waitForHttp(health, {
      timeoutMs: 90000,
      alive: () => state.backend.child.exitCode === null,
    });
    if (!ok) {
      await fail("The backend did not come up on port 8000.", `See ${state.backend.log}`);
      return;
    }
  }

  // Dashboard: our own node, a free port.
  setStatus("Starting the dashboard");
  state.dashboardPort = await servers.pickPort();
  try {
    state.dashboard = servers.startDashboard({
      dashboardDir: dashboardDir(),
      port: state.dashboardPort,
      logDir: LOG_DIR,
    });
  } catch (err) {
    await fail("The dashboard build is missing from the app.", `${err.message}\nRun \`npm run dist\` in desktop/ to rebuild.`);
    return;
  }
  const up = await servers.waitForHttp(dashboardUrl("/"), {
    timeoutMs: 60000,
    alive: () => state.dashboard.child.exitCode === null,
  });
  if (!up) {
    await fail("The dashboard did not come up.", `See ${state.dashboard.log}`);
    return;
  }

  setStatus("Opening");
  if (state.win && !state.win.isDestroyed()) await state.win.loadURL(dashboardUrl());
}

async function fail(message, detail) {
  setStatus(message);
  if (state.win && !state.win.isDestroyed()) {
    await dialog.showMessageBox(state.win, {
      type: "error",
      title: "VulcanBench",
      message,
      detail,
      buttons: ["Quit"],
    });
  } else {
    dialog.showErrorBox(message, detail);
  }
  app.quit();
}

let shuttingDown = false;
app.on("before-quit", (event) => {
  if (shuttingDown) return;
  shuttingDown = true;
  event.preventDefault();
  Promise.all([
    servers.stopChild(state.dashboard && state.dashboard.child),
    // Only a backend this app started is stopped. Stopping it never touches
    // a benchmark in flight: the supervisor adopts running jobs on restart.
    servers.stopChild(state.backend && state.backend.child),
  ]).finally(() => app.quit());
});

app.on("window-all-closed", () => app.quit());

app.on("activate", () => {
  if (!state.win && state.dashboardPort) {
    state.win = createWindow();
    state.win.loadURL(dashboardUrl());
  }
});
