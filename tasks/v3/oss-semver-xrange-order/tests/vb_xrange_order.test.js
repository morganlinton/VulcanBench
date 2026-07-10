'use strict'

// Hidden fail_to_pass tests for npm/node-semver PR #874.
// An x-range must not have a fixed (non-x) segment following an x segment:
// e.g. `1.x.5` (fixed patch after an x minor) or `x.1` (fixed minor after an
// x major) are invalid and validRange must return null. At the base these
// parse to a garbage range instead of being rejected.

const test = require('node:test')
const assert = require('node:assert')
const semver = require('./index')

test('vb xrange fixed patch after x minor', () => {
  assert.strictEqual(semver.validRange('1.x.5'), null)
})

test('vb xrange fixed minor after x major', () => {
  assert.strictEqual(semver.validRange('x.1.2'), null)
})

test('vb xrange fixed patch after x major minor', () => {
  assert.strictEqual(semver.validRange('x.x.1'), null)
})
