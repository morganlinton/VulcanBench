// Hidden pass-to-pass guards: option-less behavior is unchanged.
'use strict';
const { test } = require('node:test');
const { RuleTester } = require('./lib/api.js');
const rule = require('./lib/rules/no-unmodified-loop-condition.js');

test('vb ternary condition still grouped without options', () => {
  new RuleTester().run('no-unmodified-loop-condition', rule, {
    valid: [
      // One modified operand is enough when the condition is a ternary and no
      // option is given (the historical grouping behavior).
      'let a, b; while (a ? b : false) { b = update(); }',
    ],
    invalid: [],
  });
});

test('vb unmodified plain condition still reported', () => {
  new RuleTester().run('no-unmodified-loop-condition', rule, {
    valid: [],
    invalid: [{ code: 'let x; while (x) { doSomething(); }', errors: 1 }],
  });
});

test('vb modified plain condition still valid', () => {
  new RuleTester().run('no-unmodified-loop-condition', rule, {
    valid: ['let x; while (x) { x = step(); }'],
    invalid: [],
  });
});

// Rejected at base too (schema []) and after (additionalProperties): a guard.
test('vb unknown option properties are rejected', () => {
  const rt = new RuleTester();
  let threw = false;
  try {
    rt.run('no-unmodified-loop-condition', rule, {
      valid: [{ code: 'let x; while (x) { x = step(); }', options: [{ bogusOption: true }] }],
      invalid: [],
    });
  } catch {
    threw = true;
  }
  if (!threw) {
    throw new Error('additionalProperties must stay rejected');
  }
});
