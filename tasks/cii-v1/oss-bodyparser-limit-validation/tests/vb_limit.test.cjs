// Hidden fail-to-pass tests: invalid limit options must throw a TypeError at
// middleware creation, for every parser type.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert');
const bodyParser = require('./index.js');

const parsers = ['json', 'raw', 'text', 'urlencoded'];
const invalid = ['foo', NaN, true, {}];

test('vb invalid limits throw TypeError at creation for every parser', () => {
  for (const name of parsers) {
    for (const limit of invalid) {
      assert.throws(
        () => bodyParser[name]({ limit }),
        TypeError,
        `${name} must reject limit ${String(limit)}`
      );
    }
  }
});

test('vb invalid limit is never silently unlimited', () => {
  // The base behavior turned invalid limits into "no limit"; creation must
  // fail instead of quietly parsing unbounded bodies.
  assert.throws(() => bodyParser.json({ limit: 'not-a-size' }), TypeError);
});
