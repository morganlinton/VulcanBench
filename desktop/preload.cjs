// Exposes only the startup status to the splash page. The dashboard pages
// themselves get nothing from here: they talk to the backend over HTTP as in
// a browser.
"use strict";

const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("vulcanbench", {
  status: () => ipcRenderer.invoke("status"),
  onStatus: (cb) => {
    ipcRenderer.on("status", (_event, text) => cb(text));
  },
});
