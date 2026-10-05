# Contract and verifier crosswalk

| Behavior group | Public contract requirement | Compiling controls |
| --- | --- | --- |
| typed_dispatch | Existential payload selection, sorted-independent wire tags, heterogeneous nesting and shape identity | first-match-only also breaks ambiguity/order |
| projection_order | Each projector once in declaration order, ambiguity before payload, no known-tag fallback collision | first-match-only, known-tag-collision |
| unknown_forwarding | Opaque copying on encode, aliasing on decode, empty payload, bounded heap allocation | copy-unknown |
| framing_errors | Header/bounds/tag/length/trailing precedence, absolute offsets, nested returned errors | omit-trailing-check |
| callback_transactions | Success commit, returned error rollback, physical identity and rollback for public callback exceptions | omit-exception-rollback, swallow-public-decode |
| native_readers | Single-frame reader, concatenation, offset writer, stream/header composition, vtag rejection | reader-consumes-remainder |
| bounded_payloads | Exact native Atom views, local cursor, nested error offset/path retention | payload-offset-lost |
| schema_precedence | Invalid top-level/nested schemas before bounds and user callbacks, all operation entry points | Validation independently rejects invalid schema before calls |

Ordinary bin_prot operation and supplied interface/dependency/build/fixture
hashes are regression guards. Every public baseline behavior group fails
because Union is a stub, while both guards pass. Gold passes all groups/guards
in three fresh offline workspaces. CONTROL_AUDIT.json records intended semantic
sensitivity for each compilable control, without treating build errors as signal.
