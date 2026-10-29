# cli11-toml-config-round-trip: pre-registered design

Status: F5 wave 1 replacement candidate for slot F5-cpp, after
boosturl-path-compare-normalize was rejected at the gate on 2026-10-28
(docs/frontier-v5/GATE-LOG.md). Drafted 2026-10-28. Base and held-out
checks are measured (below); the controls, probes and the Harbor oracle
are not yet run.

## Why this candidate

Boost.URL fell to GPT-6 Astra 2 of 2 because its held-out behaviour has a
public specification (RFC 3986 section 5.2.4) that an agent can turn into a
differential oracle inside the sandbox, and both attempts did exactly that.
The C++ reserves in WAVE1-F5.md share the weakness: ada-url ships the full
WPT URLPattern data (`tests/wpt/urlpatterntestdata.json`, refreshed by a
`wpt-updater` workflow) in the same test binary the held-out checks would
use, fkYAML's block scalars have the YAML spec and yaml-test-suite, and
charconv's printing can be checked against libc `printf`. They were set
aside on paper, without building.

CLI11's config reader and writer (`include/CLI/impl/Config_inl.hpp`,
`ConfigBase::from_config` and `ConfigBase::to_config`) were rank 5 in the
C++ survey (sourcing/wave1/cpp.md). Its input looks like TOML or INI but its
behaviour is CLI11's own: how a section header with a trailing comment, a
docstring-style comment, a multiline value next to an indented key, a
Join-policy option whose values contain the delimiter, or an option that
belongs to a `+` option group or a fallthrough parent is read and written
back. There is no external specification or conformance data for those
rules; the only reference is the library's own tests, of which the
held-out ones are not in the workspace. That is the property javaparser
has (it held Astra 0 of 3), and the property the C++ slot now needs.

Risk recorded up front: the gold is small (about 150 changed lines in one
file) and the survey estimated 3 to 5 expert hours. The first-wave rejects
were of that size. What the slot gains is the absence of an oracle; whether
that is enough is what the gate measures.

## Source and base

- Upstream: https://github.com/CLIUtils/CLI11 (BSD-3-Clause), pin
  `1764a057b760e8873eab03f4635b4aaed5303a04` (main, 2026-10-22, version 2.7.2).
- Fixes reverted in the base, `include/CLI/impl/Config_inl.hpp` only:

  | Part | Upstream | Merged | Cause |
  | --- | --- | --- | --- |
  | p1361 | 45b99410 (#1361) | 2026-07-01 | a multiline value after an indented key is cut at the wrong offset |
  | header | df42e6dd (#1369), hunk 3 | 2026-07-01 | a section header followed by a comment is not recognized as a section |
  | docstring | df42e6dd (#1369), hunk 2 | 2026-07-01 | a `"""..."""` comment that opens and closes on one line swallows the rest of the file |
  | writer-join | df42e6dd (#1369), writer hunks | 2026-07-01 | a Join-policy value containing the delimiter is written so it reads back as more values (off by two in the delimiter count); also the group header comment char and the `clean_name_string` move |
  | p1451 | e7eff2f4 (#1451) | 2026-09-17 | options of a `+` option group or a fallthrough parent are written twice |
  | p1463 | e13f511a (#1463) | 2026-10-09 | multiline string content is altered (quotes and escapes processed again, closing quotes followed by text, binary escapes) |

- Kept in the base (fixed there): #1369's formatter and ExtrasError parts,
  #1364 (StringTools edge cases; one item is undefined behaviour, which
  would make a held-out check crash rather than fail), and #1432 (it
  rewrites existing test expectations). The validator fixes merged since
  2026-10-14 (#1470 to #1479) are independent one-liners outside the
  subsystem and also stay.
- Built as `git revert` of the four commits (three-way) at the pin, then
  #1369's non-config files restored from the pin; the visible tests are the
  pin's tests minus the cases the reverted commits added (and
  `TomlQuotedSectionTrailingComment`, added beside them by #1386).
- Parts: the four fixes in upstream order on top of each other end exactly
  at the pin. Each part also applies alone to the base; p1451 alone uses a
  rebased diff that leaves #1369's `commentChar` in the group header to the
  writer-join part.

## Held-out checks (measured 2026-10-28, v5-cfamily, arm64)

The pin's `tests/ConfigFileTest.cpp` (207 Catch2 cases) built against the
base fails 11 cases and the pin passes all 207:

| Part | Fail-to-pass cases | Added with the fix, already passing at base |
| --- | --- | --- |
| p1361 | StringBased: TomlMultiLineStringIndented | |
| header | TomlSectionTrailingComment, TomlQuotedSectionTrailingComment | |
| docstring | TomlDocStringCommentOneLine | |
| writer-join | IniJoinEmbeddedDelimiterRoundTrip | |
| p1451 | TomlOutputPlusGroupNoDuplication, TomlOutputFallthroughNoDuplication | TomlOutputPlusGroupNamedSubcommand, TomlOutputPlusGroupNestedGroup, TomlOutputPlusGroupCustomOptionGroup |
| p1463 | TomlMultiLineStringPreservesContent, StringBased: TomlMultiLineStringTrailingText, StringBased: TomlMultiLineStringExtraQuotes, TomlOutputMultilineStringWithQuotes | StringBased: TomlMultiLineStringTextAfterQuotes, StringBased: TomlMultiLineCommentShortLine, StringBased: TomlMultiLineStringBinary |

The six cases that pass at base are guards a partial fix can break. The
visible suite (24 test programs) passes at base. The whole suite builds in
about 30 seconds with Catch2 v2.13.10 vendored (the header CLI11 pins,
sha256 3725c0f0...8355170).

## Pre-registered naive fixes (written 2026-10-28, before any control run)

| Control | What it does | Predicted held-out effect | Predicted guards |
| --- | --- | --- | --- |
| no-quote-processing | to stop multiline content being altered, skip `process_quoted_string` for every value, not only for values the multiline reader already decoded (#1463's body: a fix that disables quote processing breaks single-line quoted values, hence the flag) | clears some of p1463's checks | `StringBased: Quotes` and other single-line quoted-value cases |
| skip-nameless-groups | to stop `+` group options being written twice, skip every nameless option group in `to_config`'s subcommand loop (#1363's body: "merging them into the parent would duplicate options") | clears TomlOutputPlusGroupNoDuplication, not the fallthrough case | `TomlOutputOptionGroup`, `IniOutputOptionGroup` (plain option groups are not merged into the parent, so their options disappear) |

Floor 8 by route (a) needs one of these to build, clear at least one
held-out check and regress at least one guard. If neither does, that is
recorded and the candidate fails floor 8; no other naive fix is searched
for.

## Still to do before any gate run

Controls (floors 1 to 7 and the naive fixes) with `run_controls.py`; the
verifier (pristine tree with the pin's tests, only `include/CLI` copied
from the agent, Catch2 JUnit per case, a must-fail sentinel per test
program, pinned case count); cheating probes; base 0 and gold 1 three
times; the Harbor oracle and nop paths; the symptoms-only instruction.
