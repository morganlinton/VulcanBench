// Hidden fail-to-pass tests: no-unmodified-loop-condition gains a
// checkConditionalExpressions option. At base the rule's schema is [], so any
// options object is rejected — every test here fails at base.
'use strict';
const { test } = require('node:test');
const { RuleTester } = require('./lib/api.js');
const rule = require('./lib/rules/no-unmodified-loop-condition.js');

test('vb option is accepted and default-off behavior is explicit', () => {
  new RuleTester().run('no-unmodified-loop-condition', rule, {
    valid: [
      {
        code: 'let a, b; while (a ? b : false) { b = update(); }',
        options: [{ checkConditionalExpressions: false }],
      },
    ],
    invalid: [],
  });
});

test('vb option flags unmodified ternary operands', () => {
  new RuleTester().run('no-unmodified-loop-condition', rule, {
    valid: [],
    invalid: [
      {
        // Only b is ever modified; with the option on, the ternary no longer
        // shields the unmodified a.
        code: 'let a, b; while (a ? b : false) { b = update(); }',
        options: [{ checkConditionalExpressions: true }],
        errors: 1,
      },
    ],
  });
});

test('vb option leaves fully modified ternary conditions valid', () => {
  new RuleTester().run('no-unmodified-loop-condition', rule, {
    valid: [
      {
        code: 'let a, b; while (a ? b : false) { a = step(); b = update(); }',
        options: [{ checkConditionalExpressions: true }],
      },
    ],
    invalid: [],
  });
});
