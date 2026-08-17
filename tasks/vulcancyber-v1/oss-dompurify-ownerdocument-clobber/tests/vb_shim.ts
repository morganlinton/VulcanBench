// Define the build-time VERSION global before purify.ts is evaluated.
// (Imported first so it runs before DOMPurify's module-eval reads VERSION.)
;(globalThis as any).VERSION = '0.0.0-vb'
export {}
