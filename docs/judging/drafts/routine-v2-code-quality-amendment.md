# Draft amendment: the same protocol on the private Routine v2 suite, in five languages

Status: DRAFT, September 26, 2026. Not approved and not frozen. No judge call
has been made under it. On approval it is appended to
`docs/judging/code-quality-maintenance-v3.md` as the next amendment number
(v3.16 unless another amendment, such as GPT-6 Luna and Sol on Frontier v4,
is frozen first), and its runner and controls are built and frozen then.

Nothing in this draft names a Routine v2 task. Task content stays in the
private VulcanRoutine repository.

## What this amendment is for

VulcanBench Routine v2 is the private successor to Routine v1: 25 routine
tickets, five each in Python, JavaScript, Rust, C++ and C, on realistic small
repositories (1,500 to 4,000 lines). The owner decided on September 25, 2026
that Code quality has full parity across the five languages: every task is
scored on the same four factors and the same 33% Code quality weight, rather
than dropping Code quality outside Python.

The v3 protocol was built and calibrated on Python only. Its rubric does not
depend on the language, but three things do, and this amendment changes
exactly those three:

1. **Evidence.** The judge sees "all Python source, Markdown documentation,
   and changed text files" (v2 evidence rule). On a C repository that is the
   Markdown and the changed files only, so a C submission would be judged
   with less context than a Python one.
2. **Calibration.** The ten held-out controls are Python implementations of
   one ledger specification. Passing that exam shows a judge can see the
   construct in Python. It shows nothing about Rust or C.
3. **Deterministic signals (L4).** `readability_signals.py` parses Python.
   L4 carries no weight, so this changes no score; it changes what is reported.

## What does not change

Rubric v3 (the reader model given verbatim, the six dimensions, anchors,
sub-score arithmetic), response schema, excerpt rules as amended through
v3.3, the system text, blinding, the locked weights (Functional 50, automated
quality 8.5, security 8.5, Code quality 33), the one-gate 0.5 allowance of
v3.2, the single-panel rule, the seed, the repeat counts, and every gate
threshold. The L2 ruling of v3.8 applies unchanged: routine tasks have no
legacy quirks, each freezes an empty quirk key, the pre-registered
zero-denominator rule moves L2's share to L1, and Routine v2 Code quality is
the L1 reviewed score of the passing panels. No probe or match call is made
on submissions.

Automated quality and security for the four new languages come from the
analyzers in harness PR #158 (clang-tidy, clang static analyzer and cppcheck
for C and C++; a pinned ESLint for JavaScript; corrected clippy and fmt for
Rust). They are part of the 8.5% and 8.5%, not of this protocol.

## Change 1: evidence in the task's language

The evidence rule becomes, for Routine v2 submissions in every language:

> Provide the task issue, the complete saved patch, and the reconstructed
> final repository: every non-test source file in the task's language, the
> build manifest (`pyproject.toml`, `package.json`, `Cargo.toml`,
> `CMakeLists.txt` or `Makefile`), all Markdown documentation, and every
> text file the patch changed, including test files the patch changed.

Source suffixes by language: Python `.py`; JavaScript `.js`, `.mjs`, `.cjs`;
Rust `.rs`; C `.c`, `.h`; C++ `.cc`, `.cpp`, `.cxx`, `.h`, `.hh`, `.hpp`,
`.hxx`. Everything else in the v2 rule stays: binary files by hash and
length, never hidden tests, gold patches, transcripts, grades, labels,
timing or cost.

Why unchanged test files are left out: Routine v2 repositories carry visible
test suites of several hundred lines, the judges rate the source a maintainer
inherits, and the tests the candidate wrote or edited are still included
because they are changed files. Python tasks follow the same rule so that the
five languages are judged on the same footing within Routine v2; this differs
from v3.8 (Routine v1 included every Python file), which is one reason the two
suites' Code quality is never compared.

Size: on the 35 Routine v2 candidates the evidence without unchanged tests is
52 to 83 KB (median 68 KB), about 3.5 times the Frontier v4 median of 19 KB.
`prepare` refuses to freeze a submission whose evidence exceeds 200 KB rather
than truncating it, and the freeze record lists every evidence size.

## Change 2: the calibration exam in every language

Each judge takes the exam in each language whose submissions it will score.

**Python.** The frozen v3 exam, byte for byte: controls 0 to 9, five
repeats, five pairs in both orders, the probe and match controls, gates 1 to
16 (80 calls). Keeping the probe gates (16) is deliberate even though L2 does
not apply on routine tasks: v3.8 and v3.14 kept them, and dropping a gate
that a judge has failed before (Grok 4.6 under v3.9) is what this protocol
warns against.

**JavaScript, Rust, C++ and C.** Ten new controls per language, in
`docs/judging/controls-v3/<language>/`, translating the ten Python controls
role for role:

| # | Role, as in the Python set | Translation rule |
| --- | --- | --- |
| 0 | Clear reference | Idiomatic for the language: its naming convention, its error mechanism (`throw new Error` in JS, a `Result` and error enum in Rust, an exception type in C++, an error code with a line-number out-parameter in C) |
| 1 | Compressed | Multi-statement lines, single-letter names, unexplained constants |
| 2 | Formatted | Control 1 through the language's standard formatter only: Prettier (JS), rustfmt (Rust), clang-format with a frozen style file (C, C++); versions pinned |
| 3 | Verbose, duplicated | Fee policy copied three times, narrating comments |
| 4 | Needless abstraction | The language's own over-abstraction idiom (class hierarchy, trait objects, virtual interfaces, function-pointer registry) for sixty lines |
| 5 | Misleading comments | Control 0 with doc comments that contradict the code |
| 6 | Narrated | Control 2 with a comment restating every line |
| 7 | Legacy quirk | Control 0 plus the `_suspense` ordering, explained in place |
| 8 | Instruction attack | Control 0 with the same reviewer-directed comment text as the Python control |
| 9 | Global state | Control 0 with records and totals in hidden module state (module-level `let`, a `thread_local!` or `static` Mutex, a namespace-scope global, a file-scope `static`) |

The ledger specification is translated per language (file name, function
names in the language's convention, error mechanism) and is otherwise the
same text; control 7 adds the same quirk sentence. A per-language verifier
compiles or loads every control and checks it against the shared behavioural
vectors (the Python verifier's sample input and expected balances, fees and
rendering), so every rating difference inside a language is a
maintainability difference. The controls are author-written, public (they
contain no task content) and hashed at freeze like the Python set.

Built September 26, 2026 (`docs/judging/controls-v3/MULTILANG.md`, one
directory per language, `verify_controls_multilang.sh` runs all four). Every
control compiles warning-free under the language's strict flags (C also
under AddressSanitizer and UndefinedBehaviorSanitizer) and reproduces the
Python verifier's vectors plus fee and rendering checks; control 2 is the
pinned formatter's output of control 1 byte for byte (Prettier 3.9.9, rustfmt
1.9.0, clang-format 23.1.2); controls 5, 7 and 8 differ from control 0 and
control 6 from control 2 only as their roles require. Translation notes, so
the differences from the Python set are on the record:

- Control 3 carries a verbose fee function in every new language, because
  the shared driver calls it; the fee policy therefore appears three times
  there and twice in the frozen Python control 3, which has none. The
  duplication construct gate 6 tests is present in both.
- Integer division: C, C++ and Rust round toward zero where Python's `//`
  rounds down, so each control corrects for negative card amounts (Rust uses
  `div_euclid`), keeping the arithmetic identical to Python's.
- Rust returns an error where Python raises on a non-integer amount, so its
  error enum has a third variant, identical in all ten Rust controls. C has
  only the two specified codes and reports limit and amount errors as the
  field-count error with the line number.
- Size follows each language's boilerplate: non-blank lines of control 0 are
  40 (Python), 59 (JavaScript), 107 (Rust), 93 (C++) and 156 (C). The order
  of sizes across the ten roles matches the Python set (control 1 smallest,
  controls 3 and 4 largest).

Per new language: ten controls, five repeats, five pairs in both orders, 60
calls; gates 1 to 15 with the Python thresholds and the one-gate allowance
applied per language. Gate 16 (probe) is Python-only and gate 17 (reader)
stays dropped, as since v3.3. Total per judge: 80 plus 4 times 60, 320
calls.

A cheaper variant is recorded for the owner's choice: three repeats and
three pairs per new language (36 calls, 224 per judge). It is not
recommended; v3.2 moved from three to five repeats because three-review
means produced single-gate failures that pointed at the instrument.

## Change 3: eligibility is per language

- A judge scores a language's submissions only if it passed that language's
  exam under this amendment. The single-panel rule applies per language:
  both pass, equal weight; one passes, that language's published L1 is the
  passing judge's and the failed judge is disclosed as a sensitivity table;
  both fail, that language has no Code quality.
- If a language has no Code quality, its five tasks' combined scores use
  the scorer's existing re-normalization over the factors present
  (functional, automated quality, security). This applies to every model
  and level alike, so rankings within Routine v2 stay comparable, and every
  card says which languages carry Code quality and which do not.
- No judge retakes a language exam it failed under this amendment.

## Change 4: signals

L4 is computed for Python submissions as before and reported as not
computed for the other four languages. It carries no weight, and no gate
uses it outside Python (gate 15 compares control ratings, not signals).

## Judges and neutrality

Muse Spark 1.3 (Meta, Standard tier, medium) and Grok 4.6 (xAI, Cursor,
medium), the Routine v1 pair under the same pinned binaries. Both passed the
exam with no allowance under v3.14 and v3.15. Both are neutral for
Anthropic, OpenAI, Cognition and Z.ai submissions.

If the Routine v2 population includes a Meta or xAI model (for example Grok
4.7, queued for the other suites), the same-family judge's scores for that
model are a disclosed sensitivity panel only, and that model's Code quality
is the other judge's score alone, labelled single-judge on every card. The
owner may instead name a third neutral judge for that model before freeze;
the draft does not choose one.

## Population

Every board model at every effort level its harness offers, one attempt per
task and level, through its own CLI on the subscription, as the charter
requires. The exact model list is fixed by the owner before the sweep and
recorded in the population record at freeze. Runs that did not finish, or
that changed no recognized source file, are excluded and listed with the
reason, as under v3.11. Routine v2 admission-gate runs (Claude Sonnet 5) are
never judged.

## Comparability

Routine v2 Code quality is never placed on one axis with Routine v1 or
Frontier v4 Code quality. Within Routine v2 it is published per model and
level, and per language, so a reader can see whether a model's Code quality
holds up outside Python. A cross-language average is shown only with the
per-language values beside it, because the five languages were calibrated
separately.

## Cost and scale

- Calibration: 320 calls per judge, 640 in all, before any counted call.
- Population: with eight models at their usual levels (about 36 cells), about
  900 submissions, each reviewed once per judge plus the v3 repeat and
  pairwise diagnostics, at about 3.5 times Frontier v4's evidence size.
  Subscription quota, not money, is the constraint; the runner resumes
  across quota windows as v3.8 did.

## Runner and records

`harness/maintenance_review_v3NN.py`, derived from the v3.14 runner, with:
the language-aware evidence builder; the controls, specification and
verifier per language; per-language calibration stages and gate evaluation;
the per-language eligibility rule in the summary; the protocol document
frozen as a copy inside the run directory. The run directory is
`judging/code-quality-maintenance-v3NN` in the private VulcanRoutine
repository; the public tree gains only the runner, the controls and this
amendment text.

## Order of work

1. Owner approves or amends the decisions below.
2. Write the 40 controls and four specifications; the per-language verifier
   passes; pin Prettier (JS formatter) alongside the pinned ESLint.
3. Build the runner; offline tests for evidence, per-language gates and the
   eligibility rule; dry run on the Routine v2 reference fixes with fake
   review scores, as v3.8 did.
4. Freeze, calibrate both judges in all five languages.
5. Judge the population after the Routine v2 sweep; publish aggregates only.

## Decisions for the owner

Settled September 26, 2026 (owner: "go with your recommendations"):

1. **Exam size for the new languages**: five repeats and five pairs, 60 calls
   per language, 320 per judge.
2. **Evidence**: non-test source in the task's language, the build manifest,
   Markdown, and every changed file; unchanged test files are left out.
3. **A language whose exam both judges fail**: published without Code
   quality, combined score re-normalized over the factors present, for every
   model alike, and disclosed on every card.
4. **Same-family submissions**: the other judge alone, labelled
   single-judge; the same-family judge is a disclosed sensitivity panel.
5. **Population**: not recommended in the draft, so the default is the
   Routine v1 roster (GPT-6 Astra, GPT-5.6 Terra, Luna and Sol, GPT-5.5,
   Claude Fable 5.1, Claude Opus 5.5, Devin SWE-2), each at every level its
   harness offers. GPT-6 Luna and Sol and Grok 4.7 are additions for the owner
   to confirm before the sweep; Grok 4.7 would fall under decision 4.
