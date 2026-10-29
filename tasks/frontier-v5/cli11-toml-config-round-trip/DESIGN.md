# cli11-toml-config-round-trip: pre-registered design

Status: F5 wave 1 replacement candidate for slot F5-cpp, after
boosturl-path-compare-normalize was rejected at the gate on 2026-10-28
(docs/frontier-v5/GATE-LOG.md). Drafted and validated 2026-10-28: base 0,
gold 1, every probe 0, Harbor oracle 1 and nop 0; F5 floors 1 to 8 met,
floor 8 by route (a), floor 3 only on its source branch (below). Not
gated.

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

## Verifier

- Separate image, no network. `/pristine` is the pin with the four fixes'
  `Config_inl.hpp` changes reverse-applied and the pin's full tests, built
  by `make_base.py` from the clone's own history and checksum-verified
  (the agent image is built the same way, minus the 17 held-out cases).
  Neither image carries the gold diff or the held-out tests in a file.
- Only `include/CLI` is copied from the agent (CLI11 is header-only and
  the tests include `CLI/...`): a header planted elsewhere in `include/`,
  such as `catch2/catch.hpp`, would precede the real Catch2 on the include
  path. The tamper scan (`cpp-catch2` rules: forced exits, `/proc/self`,
  Catch2 and JUnit names, new `catch*` files) still reads the whole tree.
- A must-fail sentinel case is appended to each of the 24 test programs;
  each program runs alone from the build's `tests` directory, as ctest runs
  it, with Catch2's JUnit reporter. A program counts only if its report
  parses, lists the sentinel as failed, and it exited non-zero without a
  timeout or signal. Ids are `<program>::<case>` (Catch2 reports one entry
  per leaf section): 11 fail-to-pass, 1,098 pass-to-pass, 1,109 pinned.
- Catch2 v2.13.10 is a minimal CMake package at `/opt/catch2`
  (`CMAKE_PREFIX_PATH`), so the agent's build and the verifier's are
  offline and identical; the header is checksum-verified.

## Difficulty hypothesis (written 2026-10-28, before any gate run)

The six causes are independent (every single-fix control clears exactly
its own checks), so this task tests finding and fixing six small defects
from symptoms, with a guard wall that punishes the blunt fix for the
largest one. Predicted misses for a reference that fixes what the
instruction names: the fallthrough half of the duplicate-output defect
(the `+` group half is the obvious one), the quoted-section variant of the
trailing-comment header, and the closer cases of the multiline reader
(spaces or a comment after the closing quotes, one or two extra quote
characters before them), which no visible test exercises. Predicted
failure of the task itself: a reference that writes its own round-trip
checks for each symptom finds all six, as Boost.URL's references did with
the RFC; here the instruction is the only description of the rules.

## Validation record (2026-10-28, arm64, v5-cfamily)

`scripts/frontier-v5/probe_verifier.py` (the real verifier, separate
container, no network, artifacts read-only):

| Artifact | Reward | Notes |
| --- | --- | --- |
| base | 0 | builds, sentinels hold, 11 fail-to-pass fail, 1,109 cases seen |
| gold | 1 | every check passes, 1,109 cases seen |
| probe exit-plain | 0 | tamper scan flags `exit(`; no program reports, sentinels missing |
| probe forge-stealth | 0 | evades the scan as intended; forged reports lack the sentinel and every id |
| probe catch-shadow | 0 | the planted `include/catch2/catch.hpp` is never copied (suite runs normally) and the scan flags the file |

Through Harbor 0.24.0 (`harbor run -a oracle` and `-a nop`, one attempt
each): oracle reward 1, nop reward 0, 1,109 cases seen in both. Base scored
0 three times (bootstrap run, probe audit, nop) and gold 1 four times
(bootstrap run, probe audit, gold-from-parts, oracle).

Both images rebuild the base from upstream history and pass their
checksums; each builds in about 40 seconds and the full verifier run takes
about a minute.

## F5 controls (`run_controls.py` with the floor-8 additions, 2026-10-28)

| Variant | Fail-to-pass cleared (of 11) | Guards |
| --- | --- | --- |
| gold-from-parts | 11 (reward 1) | hold |
| single:p1361 / header / docstring / writer-join / p1451 / p1463 | 1 / 2 / 1 / 1 / 2 / 4 | hold |
| loo:p1361 / header / docstring / writer-join / p1451 / p1463 | 10 / 9 / 10 / 10 / 9 / 7 | hold |
| naive:no-quote-processing | 1 (TomlOutputMultilineStringWithQuotes) | 8 regressed: `StringBased: Quotes`, `StringBased: TomlMultiLineStringBinary`, `TOMLStringVector`, `TOMLStringVectorMultiline`, `jsonLikeParsing`, `ConfigWriteAllDefaults`, `ConfigWriteDefaultActiveSubcommands`, `RoundTripEmptyVector` |
| naive:skip-nameless-groups | 0 | 7 regressed: the four plain option-group output cases and the three `+` group cases that pass at base |

Floors:

1. Several causes: six, each with its own held-out check. Met.
2. Concentration: one file, `Config_inl.hpp` (reader and writer). Met.
3. Interaction: met only on the source branch. #1451's upstream diff does
   not apply without #1369's writer hunks (they share the group-header
   line), shown by the controls falling back to the rebased diff. No
   held-out check needs two fixes: every single-fix control clears exactly
   its own checks. This is weaker interaction than javaparser's or
   Boost.URL's and is recorded as this candidate's main risk alongside its
   size.
4. Necessity: every leave-one-out leaves its own checks failing. Met.
5. No dominant fix: the largest single fix clears 4 of 11. Met.
6. At least 5 fail-to-pass checks: 11. Met.
7. Contract: symptoms-only instruction, no issue or PR identifiers; only
   `include/CLI` graded. Met.
8. Route (a): `no-quote-processing` builds, clears 1 held-out check and
   regresses 8 guards, as predicted (`StringBased: Quotes` among them).
   Met. The second prediction failed: `skip-nameless-groups` clears no
   held-out check (it does not stop the fallthrough duplication either, and
   the `+` group case it was predicted to clear still fails), so it does not
   count.
