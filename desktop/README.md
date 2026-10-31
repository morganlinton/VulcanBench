# VulcanBench Mac app

A window on the local run GUI (see [docs/GUI.md](../docs/GUI.md)). The app
starts the backend from this checkout (`scripts/gui_backend.sh`, loopback
only, runner enabled) and the dashboard (Next's standalone build, run with the
app's own node), then opens the Runs page. It is a launcher for the checkout
it was built from, not a self-contained install.

```bash
make app                      # builds desktop/dist/mac-arm64/VulcanBench.app
open desktop/dist/mac-arm64/VulcanBench.app
```

Pieces: `main.cjs` (Electron main: servers, window, menu), `preload.cjs` and
`splash.html` (startup status only), `lib/servers.cjs` (helpers, tested with
`npm test`), `scripts/build-dashboard.sh` (assembles the standalone dashboard
into `resources/`), `scripts/make-icon.sh` (icon from the brand mark),
`scripts/after-pack.cjs` (copies that dashboard into the app's Resources, since
electron-builder's extraResources drops node_modules), `electron-builder.yml`
(packaging).

Logs: `~/Library/Logs/VulcanBench/{backend,dashboard}.log`. Set
`VULCANBENCH_REPO` to point the app at a different checkout.
