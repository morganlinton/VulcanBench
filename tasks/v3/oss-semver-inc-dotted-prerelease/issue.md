# `inc('prerelease', identifier)` mishandles a dotted prerelease identifier

`SemVer#inc('prerelease', identifier)` lets the caller pin which prerelease
identifier to increment. When that identifier is a single token it works, but a
**dotted** identifier (e.g. `'alpha.beta'`) is mishandled: instead of matching
the leading prerelease components and bumping the numeric segment that follows,
the version's prerelease is reset.

```js
const SemVer = require('semver/classes/semver')

// Expected '3.0.0-alpha.beta.5.5'; before the fix this returns
// '3.0.0-alpha.beta.0'.
new SemVer('3.0.0-alpha.beta.5.4').inc('prerelease', 'alpha.beta').version
```

The cause: the increment logic only compares the first prerelease component
(`this.prerelease[0]`) to the identifier, so a multi-part identifier never
matches and falls through to the reset path.

## Expected behavior

When `identifier` is a dotted string, `inc('prerelease', identifier)` should
treat it as a sequence of leading prerelease components. If those components
match the current prerelease prefix, increment the numeric segment immediately
after the matched prefix (or append `.0` when the following segment is not
numeric). A plain single-token identifier, and prerelease increments with no
identifier, must behave exactly as before.

## Acceptance examples

- `new SemVer('3.0.0-alpha.beta.5.4').inc('prerelease', 'alpha.beta').version` is `'3.0.0-alpha.beta.5.5'`
- `new SemVer('3.0.0-alpha.beta.5.4').inc('prerelease', 'alpha.beta.5').version` is `'3.0.0-alpha.beta.5.5'`
- `new SemVer('1.2.3-alpha.10.beta.2').inc('prerelease', 'alpha.10.beta').version` is `'1.2.3-alpha.10.beta.3'`
- `new SemVer('1.2.3-alpha.9.beta').inc('prerelease').version` is `'1.2.3-alpha.10.beta'` (no identifier, unchanged)
