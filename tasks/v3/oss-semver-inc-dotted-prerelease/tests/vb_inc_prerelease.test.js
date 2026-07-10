'use strict'

// Hidden fail_to_pass tests for npm/node-semver PR #870.
// SemVer#inc('prerelease', identifier) must handle a DOTTED prerelease
// identifier: it should match the identifier against the leading prerelease
// components and increment the numeric segment that follows. At the base the
// code only compares the single first prerelease component to the identifier,
// so a dotted identifier resets the prerelease instead of incrementing it.

const test = require('node:test')
const assert = require('node:assert')
const SemVer = require('./classes/semver')

const inc = (version, identifier) =>
  new SemVer(version).inc('prerelease', identifier).version

test('vb inc dotted alpha beta', () => {
  assert.strictEqual(inc('3.0.0-alpha.beta.5.4', 'alpha.beta'), '3.0.0-alpha.beta.5.5')
})

test('vb inc dotted alpha beta 5', () => {
  assert.strictEqual(inc('3.0.0-alpha.beta.5.4', 'alpha.beta.5'), '3.0.0-alpha.beta.5.5')
})

test('vb inc dotted alpha 10 beta', () => {
  assert.strictEqual(inc('1.2.3-alpha.10.beta.2', 'alpha.10.beta'), '1.2.3-alpha.10.beta.3')
})
