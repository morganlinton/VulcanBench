// Shared harness for the DOMPurify hidden tests: run under `tsx --test`
// against the TypeScript source directly (no rollup build), with a jsdom
// window supplying the DOM that DOMPurify needs.
//
// `./vb_shim` MUST be imported first: it defines the build-time `VERSION`
// global that the rollup `replace` plugin would otherwise inject, and
// purify.ts reads it while evaluating its `export default createDOMPurify()`.
import './vb_shim.ts'
import createDOMPurify from './src/purify.ts'
import { JSDOM } from 'jsdom'

export function freshDOMPurify() {
  const { window } = new JSDOM(
    '<html><head></head><body></body></html>',
    { runScripts: 'dangerously' }
  )
  // Match the upstream suite: alert() flips a flag rather than blocking.
  ;(window as any).alert = () => {
    ;(window as any).xssed = true
  }
  const DOMPurify = (createDOMPurify as any)(window)
  return { DOMPurify, window, document: window.document }
}
