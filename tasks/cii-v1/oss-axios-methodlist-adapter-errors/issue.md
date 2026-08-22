# Method lists are inconsistent across core, and HTTP adapter option errors throw bare TypeError

Two related problems:

1. **The supported-method list is duplicated and out of sync.** Several
   places in core hard-code their own list of HTTP methods, and they
   disagree: `defaults.headers` has no per-method slot for `options`,
   `purge`, `link` or `unlink`, so

   ```js
   instance.defaults.headers.options['X-Marker'] = 'yes'
   // TypeError: Cannot set properties of undefined
   ```

   and method-scoped headers for those methods can't be configured, while
   the per-method aliases (`axios.options()` etc.) exist. The header
   cleanup in request dispatch uses yet another inline list. There should
   be one shared, frozen method list used by defaults, dispatch cleanup,
   and config merging.

2. **Adapter option errors are bare TypeErrors.** An invalid
   `httpVersion` (`{ httpVersion: 'x' }`) and an invalid address from a
   custom `lookup` throw plain `TypeError` — the latter from inside the
   DNS callback, where nothing catches it, so a bad lookup result can
   crash the process instead of rejecting the request. Both should reject
   with an `AxiosError` carrying the bad-option-value error code, like
   other config errors.

Ordinary requests, existing per-method header slots, and error-status
handling (`isAxiosError`, `response.status`) are unchanged.
