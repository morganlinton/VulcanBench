Add a `truncate` function to node-semver.

`semver` exposes helpers like `inc`, `coerce`, and `diff`, but nothing that
drops the lower-precision portions of a version. Add a `truncate` function that
returns a new valid version string with everything "below" a given level
removed, and export it from the package entry point (`index.js`) alongside the
other functions.

Signature: `truncate(version, releaseType, options)`. `version` is a version
string (or a `SemVer` instance); `releaseType` is one of the standard release
types (`major`, `minor`, `patch`, `premajor`, `preminor`, `prepatch`,
`prerelease`).

Behavior:

- **Build metadata is always removed** (the `+...` suffix), for every release
  type.
- For a **pre-release type** (`prerelease`, `prepatch`, `preminor`,
  `premajor`), the pre-release identifiers are kept; only build metadata is
  dropped.
- For `patch`, keep `major.minor.patch` and drop any pre-release identifiers.
- For `minor`, additionally zero the patch.
- For `major`, additionally zero both the minor and the patch.
- If `releaseType` is not a valid release type, or `version` cannot be parsed,
  return `null`.

Worked examples (`truncate(version, type) -> result`):

```
truncate('1.2.3-foo', 'patch')          -> '1.2.3'
truncate('1.2.3', 'minor')              -> '1.2.0'
truncate('1.2.3', 'major')              -> '1.0.0'
truncate('4.5.6-rc2', 'prerelease')     -> '4.5.6-rc2'   // prerelease kept
truncate('4.5.6-rc2', 'patch')          -> '4.5.6'       // prerelease dropped
truncate('4.5.6+dadb0d', 'minor')       -> '4.5.0'       // build dropped
truncate('4.5.6-rc2+dadb0d', 'major')   -> '4.0.0'
truncate('1.2.3', 'fake')               -> null
truncate('not-a-version', 'patch')      -> null
```

Existing `semver` functions must continue to work unchanged.
