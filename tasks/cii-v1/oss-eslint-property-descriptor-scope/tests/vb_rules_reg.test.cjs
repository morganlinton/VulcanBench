// Hidden pass-to-pass guards: real property descriptors keep being analyzed.
'use strict';
const { test } = require('node:test');
const { RuleTester } = require('./lib/api.js');
const getterReturn = require('./lib/rules/getter-return.js');
const accessorPairs = require('./lib/rules/accessor-pairs.js');

test('vb getter-return still flags a real descriptor getter without return', () => {
  new RuleTester().run('getter-return', getterReturn, {
    valid: [],
    invalid: [
      { code: "Object.defineProperty(foo, 'bar', { get: function() {} });", errors: 1 },
      { code: "Reflect.defineProperty(foo, 'bar', { get() {} });", languageOptions: { ecmaVersion: 6 }, errors: 1 },
    ],
  });
});

test('vb getter-return valid cases stay valid', () => {
  new RuleTester().run('getter-return', getterReturn, {
    valid: [
      "Object.defineProperty(foo, 'bar', { get: function() { return 1; } });",
      'foo.defineProperty(null, { get() {} });',
      'var x = { get a() { return 1; } };',
    ],
    invalid: [],
  });
});

test('vb accessor-pairs still flags a real descriptor setter without getter', () => {
  new RuleTester().run('accessor-pairs', accessorPairs, {
    valid: [],
    invalid: [
      { code: "Object.defineProperty(obj, 'x', { set: function(value) {} });", errors: 1 },
    ],
  });
});

test('vb accessor-pairs valid cases stay valid', () => {
  new RuleTester().run('accessor-pairs', accessorPairs, {
    valid: [
      "Object.defineProperty(obj, 'x', { get: function() { return 1; }, set: function(v) {} });",
      'var o = { set a(v) { this.val = v; }, get a() { return this.val; } };',
    ],
    invalid: [],
  });
});

// Already correct at base: kept as a guard.
test('vb accessor-pairs: wrong argument position is not a descriptor', () => {
  new RuleTester().run('accessor-pairs', accessorPairs, { invalid: [], valid: [
    "Object.defineProperty({ set: function(value) {} }, 'foo', { value: 1 });",
    { code: "Reflect.defineProperty({ get() {} }, 'foo', { value: 1 });", options: [{ getWithoutSet: true }], languageOptions: { ecmaVersion: 6 } },
    { code: 'Object.defineProperties({ foo: { get() {} } }, { bar: { value: 1 } });', options: [{ getWithoutSet: true }] },
    "Object.create({ foo: { set(value) {} } }, { bar: { value: 1 } });",
  ]});
});
