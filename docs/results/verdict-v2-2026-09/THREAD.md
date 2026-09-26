# X thread, Verdict v2 with Jev 1.13.0 (ready to paste)

Suggested order: attach the card to tweet 1, the rest are text replies.
All figures from the Verdict v2 report (`docs/results/verdict-v2-2026-09/`).

## Tweet 1, the headline
*(attach `verdict-v2-jev.png`)*

We rebuilt our benchmark for System One models.

v1 tested one skill, and Jev tied "always guess" at 63%. That told us nothing.

Verdict v2: 20 kinds of judgment, 4,774 questions, every answer checked by running code or by construction.

Jev: 45.9. GPT-6 Astra: 91.7.

## Tweet 2, what it tests

12 software families: predict a program's output, spot the type error, pick the patch that passes hidden tests, find the vulnerable version, name the service behind an incident.

8 general ones: logic, word problems, tables, policy rules.

0 = best dumb strategy. 100 = perfect.

## Tweet 3, the finding

Jev recognises. It doesn't work things out.

Strong: type errors 87, incident root cause 82, logical entailment 75.
Weak: table questions 24, estimates 24, which hidden test a patch fails 1.

The gap to a reasoning model is widest exactly where the work is.

## Tweet 4, the trade

Jev answered all 4,774 questions in 0.3 s each, for $0.58 total. The reference takes about 7 s per question and reasons at length first.

Its confidence is worth something, not much: Brier skill 0.33 against 0.87 for the reference.

## Tweet 5, keeping it honest

Keeping it honest:
- each family must beat its own shortcuts (longest option, bigger patch...)
- a reference model must show each question is answerable
- 298 of 298 sampled answers re-derived independently

We changed the entry rule after the pilot. The report says why.

## Tweet 6, the link

Full report, card and data, including every family, the method and the limits:

vulcanbench.com/benchmarks/verdict-v2-jev.html

---

## Spare stats for replies

- Software sub-index: Jev 49.6, reference 86.2. General: Jev 40.4, reference 100.0 (all 1,920 general test items right).
- Size-matched patch pairs: Jev picks the one that passes the hidden tests 68.7% of the time (skill 36). In v1 it could not beat always guessing on whether a patch passes.
- On yes or no questions Jev leans yes: 73% "true" on whether an expected value is right, where the true rate is 51%.
- 95% interval on Jev's Verdict Index: 43.1 to 48.4. The reference: 90.1 to 93.1. Separated on every index.
- One shortcut we caught: on program output, the right answer was the option most similar to the others 88.7% of the time. We rebuilt the family, and the gate now checks that shortcut on every multiple-choice family.

## Caveats to keep handy (if pressed)

- The gate was amended after the pilot: the reference only needs skill 40 (no upper bound), and a family is too easy once Jev reaches 90. Logged in the decision record on September 25, before the test split was run.
- The general families have no headroom for a reasoning model: good at separating a fast decision model from a reasoning one, useless for ranking two reasoning models.
- On "which version is vulnerable", picking the shorter version scores skill 20 on the test split. Jev is scored above it, but only just.
- The reference row shows questions are answerable; it is not a leaderboard entry. Jev has no effort setting, so it has one column.
- Some families lean on one source: networkx for planted bugs, Frontier v4 binary-parity tasks for failing tests and patch pairs.
