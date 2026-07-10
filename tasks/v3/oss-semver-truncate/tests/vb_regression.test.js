'use strict'

// pass_to_pass regression guard: pre-existing semver behavior is unchanged.
// Deliberately does NOT reference the new `truncate` function, so it passes at
// the base commit (before the fix) as well as after.

const test = require('node:test')
const assert = require('node:assert')
const semver = require('./index')

test('vb existing semver unaffected', () => {
  assert.strictEqual(semver.inc('1.2.3', 'minor'), '1.3.0')
  assert.strictEqual(semver.valid('1.2.3'), '1.2.3')
  assert.strictEqual(semver.major('4.5.6'), 4)
  assert.strictEqual(semver.coerce('v2.x').version, '2.0.0')
})
