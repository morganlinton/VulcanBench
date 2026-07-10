'use strict'

// Hidden test for oss-semver-truncate (npm/node-semver PR #855).
// A net-new `truncate(version, releaseType, options?)` function must be added
// under functions/truncate.js and re-exported from index.js. Semantics:
//   - build metadata is ALWAYS dropped;
//   - a "pre*" release type (prerelease/prepatch/preminor/premajor) keeps the
//     prerelease identifiers (only build is dropped);
//   - "patch" keeps major.minor.patch and drops prerelease;
//   - "minor" zeroes patch; "major" zeroes minor and patch;
//   - an invalid release type, or an unparseable version, returns null.
// Reference values are the upstream fixtures. Run with `node --test`.

const test = require('node:test')
const assert = require('node:assert')
const truncate = require('./functions/truncate')

const check = (cases) => {
  for (const [version, type, expected] of cases) {
    assert.strictEqual(
      truncate(version, type),
      expected,
      `truncate(${JSON.stringify(version)}, ${JSON.stringify(type)}) => ${JSON.stringify(expected)}`
    )
  }
}

test('vb truncate levels', () => {
  check([
    ['1.2.3-foo', 'patch', '1.2.3'],
    ['1.2.3', 'patch', '1.2.3'],
    ['1.2.3', 'minor', '1.2.0'],
    ['1.2.3', 'major', '1.0.0'],
    ['4.5.6-rc2', 'patch', '4.5.6'],
    ['4.5.6-rc2', 'minor', '4.5.0'],
    ['4.5.6-rc2', 'major', '4.0.0'],
  ])
})

test('vb truncate prerelease types', () => {
  check([
    ['4.5.6-rc2', 'prerelease', '4.5.6-rc2'],
    ['4.5.6-rc2', 'prepatch', '4.5.6-rc2'],
    ['4.5.6-rc2', 'preminor', '4.5.6-rc2'],
    ['4.5.6-rc2', 'premajor', '4.5.6-rc2'],
  ])
})

test('vb truncate build metadata', () => {
  check([
    ['4.5.6+dadb0d', 'prerelease', '4.5.6'],
    ['4.5.6+dadb0d', 'patch', '4.5.6'],
    ['4.5.6+dadb0d', 'minor', '4.5.0'],
    ['4.5.6+dadb0d', 'major', '4.0.0'],
    ['4.5.6-rc2+dadb0d', 'prerelease', '4.5.6-rc2'],
    ['4.5.6-rc2+dadb0d', 'patch', '4.5.6'],
    ['4.5.6-rc2+dadb0d', 'major', '4.0.0'],
  ])
})

test('vb truncate invalid', () => {
  check([
    ['1.2.3', 'fake', null],
    ['fake', 'major', null],
    ['not-a-version', 'patch', null],
  ])
})
