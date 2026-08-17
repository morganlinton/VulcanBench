# DOM clobbering of `ownerDocument` can defeat in-place sanitization

When DOMPurify sanitizes an existing DOM node in place (`DOMPurify.sanitize(node,
{ IN_PLACE: true })`), it builds a `NodeIterator` to walk the subtree. To do that
it reads the walk root's `ownerDocument` and passes it as the iterator's root
document.

That read is not clobber-safe. `HTMLFormElement` implements
`[LegacyOverrideBuiltIns]`, so a form whose child is **named** `ownerDocument`
— e.g. `<input name="ownerDocument">`, or a form-associated external input — has
its `ownerDocument` property shadowed by that child. A direct `form.ownerDocument`
read then returns the child element instead of the `Document`.

When the in-place root is such a form, the iterator is constructed against the
clobbering element rather than a real document. That call raises an exception,
and on the in-place path the exception is thrown **before** the sanitize walk's
fail-closed exception barrier runs. The result: `sanitize()` blows up (or returns)
with the caller's live tree **un-neutralized** — every armed `on*` handler and
dangerous element in the subtree survives. An attacker who controls part of the
tree being sanitized in place can use this to fully bypass sanitization.

## Expected behaviour

An `ownerDocument` that has been clobbered by a form-named child must not be able
to skip the scrub. Reading the document for the walk must return the real
`Document` regardless of any per-element shadowing, and if constructing the walk
still fails for any reason, sanitization must fail **closed** — the in-place
subtree must be stripped, never left partially or wholly un-sanitized.

Ordinary sanitization of un-clobbered trees (and of string input) must be
completely unchanged.

The affected code is the node-iterator construction in `src/purify.ts`.
