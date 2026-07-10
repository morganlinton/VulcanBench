'use strict'

// pass_to_pass regression guard: prerelease increment WITHOUT a dotted
// identifier, and unrelated semver operations, are unchanged. Passes at the
// base commit and after the fix (does not depend on the dotted-identifier fix).

const test = require('node:test')
const assert = require('node:assert')
const semver = require('./index')
const SemVer = require('./classes/semver')

test('vb existing semver unaffected', () => {
  // plain prerelease increment (no identifier) keeps its existing behavior
  assert.strictEqual(new SemVer('1.2.3-alpha.9.beta').inc('prerelease').version, '1.2.3-alpha.10.beta')
  // single-identifier increment is unchanged
  assert.strictEqual(semver.inc('1.2.3-alpha.0', 'prerelease', 'alpha'), '1.2.3-alpha.1')
  // unrelated ops
  assert.strictEqual(semver.inc('1.2.3', 'minor'), '1.3.0')
  assert.strictEqual(semver.valid('1.2.3'), '1.2.3')
})
