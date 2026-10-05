# Typed serialization motivation and provenance

The earlier small typed-schema-evolution fixture measured 1/3 native complete
passes. Both replayed failures omitted cursor restoration after a constructor
mutated it and raised. Its public Error.Decode constructor-exception policy was
unspecified. The new issue resolves that policy explicitly: user exceptions,
including Tagged.Decode, propagate by identity and restore the entry cursor.
No earlier grading or score is changed.

The [bin_prot repository](https://github.com/janestreet/bin_prot/tree/2cd58a8cd74b5fa1f28d63cbc4deaf6665b82525)
provides typed writer/reader records in src/type_class_intf.ml, native Common.buf
Bigarrays, size-prefixed Utils.bin_dump and Utils.bin_read_stream. The original
extension builds on those actual APIs rather than an erased domain-specific
codec. Upstream describes typed binary storage and stream usage in its README;
Jane Street relevance is inferred from this public source only.

The v0.17.0 Git tag resolves to 2cd58a8cd74b5fa1f28d63cbc4deaf6665b82525.
The full source is preserved in the public task with no vendor pruning. New
Tagged and Tagged_stream modules and public clients are enumerated in PROVENANCE.json.
The supplied encoder is stable and guarded. Faulty controls isolate callback
exception conversion, absent rollback, whole-buffer copying, constructor timing,
incomplete frame cursor commits and Atom slice bounds. No hidden requirement,
shortened model budget or changed effort is used to manufacture difficulty.
