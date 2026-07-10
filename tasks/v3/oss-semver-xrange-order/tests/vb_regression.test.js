'use strict'

// pass_to_pass regression guard: well-formed x-ranges (x only in trailing
// positions) still expand correctly, and plain versions are unaffected. Holds
// at the base commit and after the fix.

const test = require('node:test')
const assert = require('node:assert')
const semver = require('./index')

test('vb existing xranges unaffected', () => {
  assert.strictEqual(semver.validRange('1.2.x'), '>=1.2.0 <1.3.0-0')
  assert.strictEqual(semver.validRange('2.x'), '>=2.0.0 <3.0.0-0')
  assert.strictEqual(semver.validRange('2.x.x'), '>=2.0.0 <3.0.0-0')
  assert.strictEqual(semver.validRange('x'), '*')
  assert.strictEqual(semver.validRange('1.2.3'), '1.2.3')
})
