# Replace the retired CodecCore codec; partner interchange is breaking in both directions

We are decommissioning the legacy CodecCore VX codec (a vendor binary
whose source was lost in the 2020 handover) and replacing it with
`codeccore.py`. The Python rewrite was done from `docs/SPEC.md` and looks
correct on the happy path, but the partner systems on both sides of the
interchange, which were all built against the engine's actual behavior
over many years, are breaking batch after batch:

- Partner files that the old engine decoded fine are now rejected
  (`R LEN`, `R FMT`, `R CHK` on records that used to come back as `P`
  lines), and the two sides disagree inconsistently from file to file.
- Records we encode no longer match what partners expect byte for byte:
  high-value January records are missing their audit flag, and long memos
  that used to go through are now bounced.
- Round trips are altering records: decode-then-re-encode through the new
  codec produces different bytes than the engine did, and
  encode-then-decode drops or mangles fields the engine handled.
- Batch trailers (`X` counts) no longer reconcile with partner logs.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so the
replacement must reproduce its behavior, not invoke it.

Make `codeccore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including records, check characters,
error codes, echoes, and the trailer) for any command stream, in both the
encode and decode directions, matching how the engine actually parses,
validates, encodes, decodes, and counts, wherever that differs from what
the spec says. Where the spec IS accurate, nothing may change.
