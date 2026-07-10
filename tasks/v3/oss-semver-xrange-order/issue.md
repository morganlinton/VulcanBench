# x-ranges accept a fixed segment after an `x` segment

In a semver range, an `x`/`*` wildcard segment may only be followed by more
wildcards. A range like `1.x.5` (a fixed patch after an `x` minor) or `x.1`
(a fixed minor after an `x` major) is not meaningful and should be rejected.
Instead, such ranges are silently coerced into a garbage range:

```js
const semver = require('semver')

// Expected null (invalid). Before the fix: ">=1.0.0 <2.0.0-0".
semver.validRange('1.x.5')

// Expected null (invalid). Before the fix: "*".
semver.validRange('x.1.2')
```

## Expected behavior

When expanding an x-range, if a wildcard segment is followed by a fixed
(non-wildcard) segment, the range is invalid: `validRange` must return `null`
and the comparator must not be expanded. Well-formed x-ranges, where `x` appears
only in trailing positions (`1.2.x`, `2.x`, `2.x.x`, `x`), must continue to
expand exactly as before.

## Acceptance examples

- `semver.validRange('1.x.5')` is `null`
- `semver.validRange('x.1.2')` is `null`
- `semver.validRange('x.x.1')` is `null`
- `semver.validRange('1.2.x')` is `'>=1.2.0 <1.3.0-0'` (unchanged)
- `semver.validRange('2.x')` is `'>=2.0.0 <3.0.0-0'` (unchanged)
