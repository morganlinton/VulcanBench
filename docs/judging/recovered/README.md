# Recovered frozen judge protocols

Byte-for-byte copies of the frozen `protocol.json` files from the Code quality
v3.3 and v3.4 run directories. Every later Frontier v4 judging round reads the
Grok reviewer settings from v3.3 and the Muse settings from v3.4. The run
directories themselves are gitignored and existed only on the original judging
host, which was erased; these copies come from the owner's private
pre-reset backup (2026-10-05) and are committed so the settings cannot be lost
again.

| File | sha256 | Published as |
| --- | --- | --- |
| `code-quality-maintenance-v3.3-protocol.json` | `b82da5987bb555443c9006e51e5a4d555c41858dc710140f0571bd90ed8a5eec` | `v3.3/protocol.json` in the site's `swe-v4-astra-fable51-v34/provenance.json` |
| `code-quality-maintenance-v3.4-protocol.json` | `1d80e0974526f8d825f14921c9102c8eda47ec312a0981783475149e31fd8524` | `v3.4/protocol.json` in the same file |

Both hashes equal the published values, so these are the original files. Real
tasks' quirk keys appear in them only as hashes; `ledger_key` belongs to the
synthetic calibration controls and is already public in
`harness/maintenance_review_v3.py`. Host paths under `/Users/morganlinton` are
the original host's and are kept unchanged.

What they established for v3.23 (docs/DECISIONS.md, 2026-10-08): the reviewer
settings rebuilt in `docs/judging/judge-pins-v3.json` are identical to these,
and the Muse binary hash matches. The Cursor entry pins only Cursor's launcher
script, which is the same file in every Cursor release, so it never fixed the
Cursor version.
