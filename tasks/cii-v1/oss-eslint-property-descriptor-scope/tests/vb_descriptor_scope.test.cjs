// Hidden fail-to-pass tests: getter-return and accessor-pairs must only treat
// an object as a property descriptor when it actually sits in the descriptor
// argument position of the real (unshadowed, existing) global method.
'use strict';
const { test } = require('node:test');
const { RuleTester } = require('./lib/api.js');
const getterReturn = require('./lib/rules/getter-return.js');
const accessorPairs = require('./lib/rules/accessor-pairs.js');

const valid = (rule, name, cases) => {
  new RuleTester().run(name, rule, { valid: cases, invalid: [] });
};

test('vb getter-return: wrong argument position is not a descriptor', () => {
  valid(getterReturn, 'getter-return', [
    "Object.defineProperty({ get() {} }, 'foo', { value: 1 });",
    { code: "Reflect.defineProperty({ get() {} }, 'foo', { value: 1 });", languageOptions: { ecmaVersion: 6 } },
    'Object.defineProperties({ foo: { get() {} } }, { bar: { value: 1 } });',
    'Object.create({ foo: { get() {} } }, { bar: { value: 1 } });',
  ]);
});

test('vb getter-return: shadowed global is not the real Object/Reflect', () => {
  valid(getterReturn, 'getter-return', [
    "let Object; Object.defineProperty(foo, 'bar', { get() {} })",
    { code: "function f() { Reflect.defineProperty(foo, 'bar', { get() {} }); var Reflect;}", languageOptions: { ecmaVersion: 6 } },
    'function f(Object) { Object.defineProperties(foo, { bar: { get() {} } }) }',
    'if (x) { const Object = getObject(); Object.create(foo, { bar: { get() {} } }) }',
  ]);
});

test('vb getter-return: disabled global is not a descriptor call', () => {
  valid(getterReturn, 'getter-return', [
    { code: "Reflect.defineProperty(foo, 'bar', { get() {} })", languageOptions: { globals: { Reflect: 'off' } } },
    "/* globals Object:off */ Object.defineProperty(foo, 'bar', { get() {} })",
    { code: 'Object.defineProperties(foo, { bar: { get() {} } })', languageOptions: { globals: { Object: 'off' } } },
  ]);
});

test('vb accessor-pairs: shadowed or disabled global is not a descriptor call', () => {
  valid(accessorPairs, 'accessor-pairs', [
    { code: "let Object; Object.defineProperty(foo, 'bar', { get() {} })", options: [{ getWithoutSet: true }] },
    { code: "function f() { Reflect.defineProperty(foo, 'bar', { set(value) {} }); var Reflect;}", languageOptions: { ecmaVersion: 6 } },
    'function f(Object) { Object.defineProperties(foo, { bar: { set(value) {} } }) }',
    { code: "Reflect.defineProperty(foo, 'bar', { get() {} })", options: [{ getWithoutSet: true }], languageOptions: { globals: { Reflect: 'off' } } },
    "/* globals Object:off */ Object.defineProperty(foo, 'bar', { set(value) {} })",
  ]);
});
