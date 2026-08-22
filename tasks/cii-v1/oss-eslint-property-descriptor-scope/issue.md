# getter-return and accessor-pairs false positives: any object with get/set near defineProperty is treated as a property descriptor

`getter-return` and `accessor-pairs` report getters/setters that appear
anywhere around `Object.defineProperty` / `Reflect.defineProperty` /
`Object.defineProperties` / `Object.create` calls — even when the object is
not in the property-descriptor argument position, or when the "global"
being called is not the real global at all.

False positives:

```js
// the object with the getter is the TARGET, not the descriptor
Object.defineProperty({ get() {} }, 'foo', { value: 1 });
Object.defineProperties({ foo: { get() {} } }, { bar: { value: 1 } });

// Object/Reflect is shadowed — this is not the global function
let Object; Object.defineProperty(foo, 'bar', { get() {} })
function f(Object) { Object.defineProperties(foo, { bar: { get() {} } }) }

// the global is disabled for the linted code
/* globals Object:off */ Object.defineProperty(foo, 'bar', { get() {} })
```

None of these should be treated as property descriptors. The
descriptor-position check must consider the argument index the object
occupies, and the call must reference the actual global variable
(unresolved or resolved-to-global reference, not shadowed, not turned off
in the language options).

Real descriptors keep being analyzed exactly as today: a get-without-return
inside a genuine `Object.defineProperty(foo, 'bar', { get() {} })` is still
reported by `getter-return`, and a set-without-get by `accessor-pairs`.
`no-setter-return` shares the same descriptor detection and must stay
consistent.
