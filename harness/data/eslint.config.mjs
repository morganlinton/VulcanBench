// ESLint flat config for VulcanBench's JavaScript quality and security metrics.
//
// The harness runs a pinned ESLint install (VULCANBENCH_ESLINT_DIR, set up by
// scripts/install_analyzers.sh) with this file as --config, so every run is
// scored by the same rules and versions whatever the workspace ships.
// Plugins resolve from the pinned install, not from the workspace.
//
// VULCANBENCH_ESLINT_MODE selects the rule set:
//   quality  - ESLint recommended plus a few maintainability rules
//   security - eslint-plugin-security (minus detect-object-injection, which
//              flags every bracket access) plus core eval-family rules
import { createRequire } from "node:module";

const dir = process.env.VULCANBENCH_ESLINT_DIR;
if (!dir) throw new Error("VULCANBENCH_ESLINT_DIR is not set");
const require = createRequire(dir.endsWith("/") ? dir : `${dir}/`);
const js = require("@eslint/js");
const security = require("eslint-plugin-security");

// Node and web-platform globals available in Node 22 without imports.
const nodeGlobals = Object.fromEntries(
  [
    "AbortController", "AbortSignal", "Blob", "Buffer", "BroadcastChannel",
    "clearImmediate", "clearInterval", "clearTimeout", "console", "crypto",
    "CustomEvent", "Event", "EventTarget", "fetch", "FormData", "global",
    "Headers", "MessageChannel", "performance", "process", "queueMicrotask",
    "Request", "Response", "setImmediate", "setInterval", "setTimeout",
    "structuredClone", "TextDecoder", "TextEncoder", "URL", "URLSearchParams",
    "WebAssembly",
  ].map((name) => [name, "readonly"]),
);
const commonJsGlobals = Object.fromEntries(
  ["require", "module", "exports", "__dirname", "__filename"].map((n) => [n, "readonly"]),
);

const base = [
  {
    files: ["**/*.js", "**/*.mjs", "**/*.jsx"],
    languageOptions: { ecmaVersion: "latest", sourceType: "module", globals: nodeGlobals },
  },
  {
    files: ["**/*.cjs"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "commonjs",
      globals: { ...nodeGlobals, ...commonJsGlobals },
    },
  },
];

const quality = [
  js.configs.recommended,
  {
    rules: {
      complexity: ["warn", 15],
      eqeqeq: ["warn", "smart"],
      "no-var": "warn",
      "prefer-const": "warn",
      "no-unused-vars": ["warn", { args: "after-used", caughtErrors: "none" }],
    },
  },
];

const securityRules = [
  {
    plugins: { security },
    rules: {
      ...security.configs.recommended.rules,
      "security/detect-object-injection": "off",
      "no-eval": "warn",
      "no-implied-eval": "warn",
      "no-new-func": "warn",
    },
  },
];

const mode = process.env.VULCANBENCH_ESLINT_MODE || "quality";
export default [...base, ...(mode === "security" ? securityRules : quality)];
