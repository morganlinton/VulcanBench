# no-unmodified-loop-condition: add checkConditionalExpressions option

`no-unmodified-loop-condition` treats a ternary loop condition as one group:
if **any** referenced variable in the condition is modified in the loop, the
whole condition passes. That hides real bugs — in

```js
let a, b;
while (a ? b : false) {
    b = update();
}
```

`a` is never modified, so the branch choice never changes, but the rule
stays silent because `b` is modified.

Add an option:

```js
"no-unmodified-loop-condition": ["error", { "checkConditionalExpressions": true }]
```

- With the option **on**, conditional (ternary) expressions no longer group:
  a reference in the loop condition must itself be modified, so the example
  above is reported. A ternary whose referenced variables are all modified
  stays valid.
- The default is **off**, keeping today's grouping behavior exactly —
  including for configurations that pass the option explicitly as `false`.
- The options schema accepts only this property (unknown properties keep
  being rejected), and the rule's published types are updated.
- Option-less behavior — plain unmodified conditions reported, modified
  ones valid, ternary grouping — is unchanged.
