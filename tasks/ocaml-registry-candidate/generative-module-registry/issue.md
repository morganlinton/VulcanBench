# Generative typed plugin registry

Implement Key and Registry, preserving every supplied .mli, Plugin, public
fixtures and Dune definitions. No Obj operations, unsafe casts, Marshal,
external identity primitives or concrete-payload special cases. Heterogeneous
entries must preserve their key/value type relationship; do not erase values.
Helpers and additional source modules are allowed. No global mutable registry,
key counter or cache. This is an original integration fixture, not an OSS bug.

Key.Make(P)() creates a fresh key on EVERY generative application, including
applications to the same P and identical payload types/names. Copies/repackaging
of one module retain its key identity. Key.name returns P.name. Key.same a b
returns Some Refl exactly when a and b are the SAME generated key, otherwise
None. Equality must work in both directions and provide a usable type proof
without unsafe casting. Names and payload types do not establish key identity.
First-class Key.S packages and clients with abstract payload types must work.

Registry.create gives an independent open owner. register replaces only the
same key, invalidates its prior handle and appends a new entry at the end of
registration order. Distinct keys with identical names/types coexist. find and
handle return the current typed value/handle; read returns Some original value
only while the handle remains current and its owner is open. Preserve payload
physical identity; do not serialize/copy it. remove returns true iff a live
entry existed and invalidates its handle; missing/closed returns false. Old
handles never revive after re-registration. Owners are independent.

update h f returns false without calling f for an invalid/closed handle. For
live h, call f exactly once with its current value. If h is still live on return,
store the result and return true. If f removes/replaces its entry or closes its
owner, return false and never resurrect or overwrite the replacement. Nested
updates to the SAME still-live handle are permitted; the outer result commits
last. If f raises ANY exception, propagate the same exception object, do not
store an outer result, and retain all reentrant callback mutations. No general
transaction rollback of callback effects is required.

close is idempotent, invalidates all handles and discards entries. Closed find,
handle and read return None; remove/update return false; iter invokes nothing.
register/clone on a closed owner raise Invalid_argument. transfer rejects either
closed owner before key lookup, including same-owner/missing-key calls.

clone copies registration order into a new open owner with NEW handles and the
SAME keys/physically identical payload values. Closing, replacing or updating
one owner cannot mutate the other owner's slots/handle validity. Mutations to
shared payload objects remain visible because payloads are not deep-copied.

transfer ~src ~dst key returns false for missing key in open src, leaving dst
untouched. For a present key, same-owner transfer returns true without changing
handle/order/value. Across owners, invalidate the src handle, replace and
invalidate any dst handle for that key, append a NEW dst-owned handle, and
preserve the payload object. Do not move a handle whose owner still points to src.
No user callbacks run during transfer. Other keys retain order and validity.

iter uses its rank-polymorphic visitor on a snapshot of handles taken at entry,
in registration order. Immediately before each visit, skip handles that have
become invalid/closed; otherwise pass the CURRENT typed value at that turn.
Newly added/replacement handles are not visited in this iteration. Callback
remove/replace/close and nested iter/update calls are allowed. An exception
propagates unchanged, stops remaining visits and retains callback effects.
Closed iteration is empty. Replacing a captured entry skips its stale handle,
even if the same key is reinserted during iteration.

Use the pinned native toolchain. dune build --profile release @runtest is the
public preflight; the smoke client deliberately exercises only supplied baseline
behavior. Compile and test your implementation with additional clients as needed.
