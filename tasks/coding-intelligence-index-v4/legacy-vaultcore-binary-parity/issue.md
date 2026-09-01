# Replace the retired VaultCore engine; vault audits no longer reconcile

We are decommissioning the legacy VaultCore document vault engine (a
vendor binary whose source was lost when the vendor folded in 2018) and
replacing it with `vaultcore.py`. The Python rewrite was done from
`docs/SPEC.md` and looks correct on the happy path, but the archival
pipelines that write, sweep, and audit the vault, all built against the
engine's actual behavior over many years, are misbehaving batch after
batch:

- Vault checksums are drifting after sweeps but not after reads: an
  archive drained by individual reads reconciles against the engine's
  audit logs, while the same archive processed by the nightly bulk
  sweep produces `C` sums the logs have never contained, and the drift
  compounds across later commands.
- Some rewritten documents are coming back short: documents that were
  updated in place before migration read back with digests the audit
  tooling flags as truncated content, while untouched neighbors written
  in the same batches read back fine.
- Downgrades the engine has always absorbed are now refused: pipeline
  replays that write an older-format revision over a current document,
  which the engine acknowledged with `OK` for years, come back `R VER`
  from the new store, and everything downstream of the replay halts.
- Digests disagree almost everywhere: even freshly written documents
  read back with digests that differ from what the engine logged for
  identical writes, and `OK` version counts and the batch trailers
  (`X` counts) no longer reconcile with pipeline logs either.

The spec's own header warns about this: the document has drifted, and
**the engine's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`), but it is NOT present in production, so
the replacement must reproduce its behavior, not invoke it.

Make `vaultcore.py` a drop-in behavioral replacement for the engine:
byte-for-byte identical reply lines (including digests, version counts,
checksums, error codes, and the trailer) for any command stream, across
writes, reads, bulk sweeps, and checksums, matching how the engine
actually parses, validates, stores, migrates, and counts, wherever that
differs from what the spec says. Where the spec IS accurate, nothing may
change.
