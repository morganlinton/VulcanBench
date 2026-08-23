// Hidden fail-to-pass tests: registry -> JSON Schema conversion must scale
// LINEARLY in registry size.
//
// These are the only failing tests at the base commit: the conversion is
// already correct there, just quadratic. Correctness lives in the guards, so an
// implementation that is fast but wrong scores 0.0, and one that is correct but
// slow scores 0.0 too.
import { test } from 'node:test'
import assert from 'node:assert'
import * as z from './packages/zod/src/v4/index'

const buildRegistry = (n: number) => {
  const reg = z.registry<{ id: string }>()
  for (let i = 0; i < n; i++) {
    reg.add(z.object({ a: z.string(), b: z.number(), c: z.boolean().optional() }), {
      id: `S${i}`,
    })
  }
  return reg
}

const convert = (reg: unknown) =>
  z.toJSONSchema(reg as never, { uri: (id: string) => `#/x/${id}` } as never)

const timed = (fn: () => unknown): number => {
  const t0 = Date.now()
  fn()
  return Date.now() - t0
}

test('vb registry conversion scales linearly, not quadratically', () => {
  // Machine-independent: only the RATIO is asserted. Quadratic work grows ~16x
  // when the input quadruples; linear work grows ~4x. Measured at the base
  // commit this ratio is ~16.5; with linear conversion it is ~3.
  const small = buildRegistry(500)
  const large = buildRegistry(2000)
  // Warm the code paths so JIT effects land on both measurements alike.
  convert(buildRegistry(50))

  const tSmall = Math.max(timed(() => convert(small)), 1)
  const tLarge = timed(() => convert(large))
  const ratio = tLarge / tSmall

  assert.ok(
    ratio < 8,
    `quadrupling the registry multiplied the work by ${ratio.toFixed(1)}x ` +
      `(${tSmall}ms -> ${tLarge}ms); linear conversion should be well under 8x`
  )
})

test('vb large registry converts within a generous time budget', () => {
  // At the base commit this takes ~20s; linear conversion takes well under
  // 100ms. The 6s budget sits ~60x above the fast path and ~3x below the slow
  // one, so it is robust to machine speed.
  const reg = buildRegistry(3000)
  const elapsed = timed(() => convert(reg))
  assert.ok(elapsed < 6000, `converting 3000 registered schemas took ${elapsed}ms, budget 6000ms`)
})
