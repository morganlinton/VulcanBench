# VulcanBench ComputerUse v1: the computer-use suite (charter draft)

Status: **draft, 2026-11-05**. Nothing in this document is built yet. The
suite id is `computer-use-v1` and the public name is "VulcanBench ComputerUse
v1", both fixed on 2026-11-05 (slugs and run directories are never renamed).

## Goal

A small suite of **computer-use tasks**: a person's goal, stated in plain
language, that an agent must accomplish by looking at a screen and operating
real software with a mouse and keyboard. Every task runs in a hermetic
container with a real display, is seeded with realistic state and
distractors, and is graded by a deterministic **end-state checker**, never by
how the agent got there. The suite measures whether a model can reliably and
safely operate software it has not seen, across the same agents a customer
would actually use (the raw computer-use tool through the API, Claude in
Chrome, Claude Cowork, and the equivalent products from other labs).

It exists for the same reason VulcanCyber and Voice v1 exist: the coding
suites measure one modality of agentic work, and the interesting failures
now live elsewhere. For computer use the open questions are perception (what
the model can read off a screenshot), long-horizon state tracking, recovery
from the unexpected (a modal dialog, a stale page, a mis-click), and knowing
when to stop.

## Three hard truths

1. **Trajectory matching measures imitation, not competence.** There are
   many correct ways to rename a file or file an expense. A task is graded
   only on the state of the world when the agent stops: the row in the
   database, the file on disk, the setting that changed, the email that was
   (or was not) sent. A gold trajectory exists to prove solvability, never
   to grade against.
2. **Toy apps saturate immediately.** A login form with three fields does
   not discriminate frontier models. Difficulty comes from state: seeded
   data with near-duplicates, multi-step dependencies across screens or
   apps, settings that only appear after another setting is toggled, a
   dialog that interrupts the flow, pagination that hides the target, and
   horizons of 20 to 80 actions. We build difficulty into the world, not
   into the wording.
3. **Reward hacking is the default failure mode of a checker.** If the end
   state is reachable without the UI (editing the SQLite file, writing the
   config by hand, curling the app's API), some agent will reach it that
   way, and the checker will say yes. Every checker is probed with a
   deliberate cheat before admission (see validation), and tasks that must
   be done through the UI verify the app's own event log as well as the
   final state.

## What counts as a task (three families)

- **(A) Web apps in a browser.** Self-hosted, permissively licensed
  applications (an issue tracker, a CRM, a webmail client, a wiki, a
  storefront admin, a calendar) running on a local server with no outbound
  network. Reskinned (theme, labels, layout) so screenshots of the stock
  app are no help. Goals look like office work: "move every ticket assigned
  to Priya that mentions the invoice bug into the Billing project and set
  them to high priority."
- **(B) Native desktop apps under a virtual display.** GTK, Qt and Tk
  applications, LibreOffice, a file manager, a terminal emulator, a PDF
  viewer, system-settings style dialogs. Goals exercise native widgets:
  menus, trees, tabs, drag targets, file pickers, keyboard shortcuts.
- **(C) Cross-app workflows.** One goal that needs two or more of the above:
  read a figure from a spreadsheet and enter it in the web form; download
  the attachment from the mail client and rename it per the convention in
  the wiki page; reconcile a list between two apps and report the
  discrepancies in a text file.

Roughly balanced across (A) and (B), with (C) as the long-horizon tail.

## Realism bar (every task)

- The goal is phrased as a person would say it. It names outcomes, never
  widgets, element ids, coordinates or the shape of the solution.
- The world contains distractors: records that nearly match, a second user
  with a similar name, a stale draft, a disabled button, a notification that
  arrives mid-task.
- Irreversible controls are present (Send, Delete, Pay, Empty trash) whether
  or not the task touches them. Their presence is part of the realism, and
  the safety track (below) measures what the agent does near them.
- Screens are deterministic: fonts pinned, animations off, clock frozen,
  random seeds fixed, no network. The same action sequence produces the same
  screenshots on every run.
- Nothing is copied from public computer-use benchmarks (OSWorld, WebArena,
  WindowsAgentArena, Mind2Web and their descendants). Apps may be forked
  from open source, but goals, seed data and skins are ours. Each task
  records `decontaminated` with a note on the app's provenance.

## Perception conditions

Each task runs under named input conditions, the way Voice v1 runs an item
as text and as audio. The headline delta is the **screenshot tax**: pass
rate with pixels only, minus pass rate with pixels plus an accessibility
tree.

| Condition | Agent sees |
|---|---|
| `pixels` | screenshots only, standard resolution |
| `pixels+tree` | screenshots plus an accessibility tree with element refs |
| `tree` | accessibility tree only, no image |
| `pixels-lowres` | screenshots downscaled to the provider's minimum |
| `pixels+zoom` | screenshots plus a zoom-region tool |

A suite result is only comparable within one condition. Reports show
`pixels` as the primary column and the others as ablations.

## Action space and agents under test

The agent gets the standard computer-use action set: screenshot, left and
right click, double click, drag, type, key chord, scroll, wait, plus zoom
under `pixels+zoom`. A thin adapter maps each provider's tool schema onto
that set so the tasks never change per vendor. Agents measured, each
recorded by product and version in the run metadata:

- Anthropic computer-use tool via the API (model and tool version), through
  Harbor's `computer-1` agent
- Claude Code on the Claude subscription, acting only through the suite's
  MCP computer tools (`sandbox/computer-use/vb_computer_mcp.py`: screenshot,
  click, type, key, scroll, drag, zoom over the same desktop) with its own
  tools disallowed. This is its own row, not a stand-in for the native tool:
  screenshots reach the model as ordinary images and coordinates come back
  as tool arguments
- Claude in Chrome (web family only) and Claude Cowork
- OpenAI computer-use agent via the API and through its product
- Google Gemini computer-use via the API

The product-versus-API comparison for the same model is a first-class
result (the "product tax"), matching how the coding suites compare Claude
Code to the raw API.

## Grading (deterministic, no judge)

- **Sparse reward**: a checker runs in the verifier container after the
  agent stops, with the agent's session killed and the display gone. It
  inspects state only (database rows, files, app config, the app's own
  audit or event log) and returns 1.0 or 0.0. This is the published
  pass@1 number.
- **Dense variant**: the same checker exposes an ordered sub-goal checklist
  (opened the right record, changed field X, saved, did not touch Y) and
  returns the fraction met. It is never published as a headline; it exists
  for training environments and for diagnosing where long tasks break.
- **Guards**: state that must not change (other users' records, unrelated
  files, settings outside the task) is checked exactly like `pass_to_pass`
  tests in the coding suites. Any guard failure zeroes the task.
- **No LLM judge.** "Looks done" and "is done" diverge too often on GUI
  work, and a judge reading screenshots inherits the perception problem
  under measurement.

## Safety track (reported separately, never folded into pass@1)

A subset of tasks places the agent near actions it should not take alone:

- page or document content carrying instructions aimed at the agent
  (prompt injection in an email body, a ticket comment, a PDF);
- credential, payment and government-id fields;
- irreversible controls where the goal is ambiguous about whether to use
  them ("clean up my inbox" next to Empty trash).

The correct behavior is to stop and ask, or to complete the goal without
the dangerous action. The checker records whether the dangerous state
transition happened and whether the agent's final message asks. Two
numbers are reported: injection compliance rate (lower is better) and
unsafe-action rate (lower is better), with task success alongside so a
model cannot score well by refusing everything.

## Environment contract (Harbor-native)

Tasks use the same Harbor layout as Frontier v5, so the gate runner, the
local GUI's runs page and the reward-per-trial reporting work unchanged:

```
tasks/computer-use-v1/<task>/
  task.toml          suite, family, condition list, budgets, canary
  instruction.md     the goal, as the person would state it
  environment/       Dockerfile: Xvfb, window manager, the app(s), seed data,
                     a CDP or VNC endpoint for the adapter, network off
  solution/          a gold script (Playwright or xdotool) that solves the
                     task through the UI; proves solvability, never grades
  tests/             the checker, the guard set, the cheat probes
  DESIGN.md          provenance, distractor design, what the task measures
```

Each task is a training environment as well as an eval: the container,
the action adapter and the dense checker are exactly what an RL loop needs
(reset, step, reward). The suite is designed so that a model fine-tuned on
the dense signal of one half of the tasks can be evaluated on the sparse
signal of the other half, which is the experiment the suite is built to
enable.

## Validation pipeline (the admission gate)

Every candidate passes all of these before it is measured against a model:

1. **Gold solves it**: the gold script reaches reward 1.0, three times from
   a fresh container.
2. **Base fails it**: the untouched seed state scores 0.0.
3. **Guards hold both ways**: the gold run passes every guard; a run that
   deliberately violates each guard is zeroed.
4. **Cheat probes fail**: for each task, at least two scripted shortcuts
   that reach the end state without the UI (direct database write, config
   file edit, API call) must score 0.0. If a probe passes, the checker is
   wrong, not the probe.
5. **Screen determinism**: the gold script's screenshots are pixel-identical
   across three runs, or the task documents exactly which regions vary and
   why (a clock, a cursor blink).
6. **Flake rate**: a model run that is known to pass is replayed from its
   action log three times; divergence is a task defect.
7. **Budget**: the gold script finishes in under a quarter of the task's
   time budget and step cap.

`scripts/computer-use/validate_task.py` runs these checks through Harbor and prints a
verdict per task; nothing enters `suite.lock.json` without it.

## Difficulty composition

Build 20 to 25 candidates across the three families, measure each at n=3
under the `pixels` condition against two reference agents (the current
Anthropic and OpenAI computer-use agents at admission time, recorded with
versions and dates), then compose the admitted set. Keep any well-formed
task anywhere on the spectrum; a task both references score 0 on is
desirable if it is fair. Reject tasks both references ace at every effort
level, unless kept as a deliberate easy anchor, and reject anything flaky,
ambiguous or cheatable. Re-measure when the frontier moves.

## Budgets

Adopted 2026-11-06 (`docs/DECISIONS.md`): a flat per-task wall clock of 60
minutes, recorded in `suite.json` under `pins.budget` and stamped into each
task's `[agent].timeout_sec`, plus a 200-action cap for the computer-1
reference. This departs from the 3-hour default the other suites carry
because a GUI task's gold path is seconds and its cost is vision tokens, not
compute; the evidence is in the decision entry. Sweeps run one task at a time.

## Metrics and reporting

Per run: reward, sub-goal fraction, actions taken, screenshots taken, input
tokens split into image and text, cost at list price, wall clock, condition,
agent product and version. Per suite: pass@1 and pass@k with run counts and
stderr whiskers, the screenshot tax, the product tax, and the two safety
rates. Every chart-integrity rule from the rankings chart applies: visible
`n=`, whiskers, asterisks on partial coverage, footnoted external numbers.

## Target

12 to 15 admitted tasks across the three families, each runnable under all
five perception conditions, with at least three safety-track tasks and a
genuine hard tail (at least one task both reference agents fail). Quality
over volume.

## Scope: what this suite measures, and what it does not

Measures: goal-directed operation of unfamiliar desktop and web software
from screen input, over horizons of tens of actions, graded by end state;
the cost of perception (screenshot tax) and of the product wrapper (product
tax); and behavior near irreversible and injected content.

Does not measure: mobile interfaces, games, real third-party websites
(network is off), multi-hour or multi-day work, voice or collaborative
sessions, or anything requiring a human in the loop. Composition is
difficulty-weighted, so the score is a discrimination instrument, not a
survey of office labor.

## Publication and cost rules

- The repository is public. A pushed run tag publishes every task's
  checker, seed data and gold script. Tasks whose difficulty depends on the
  agent not knowing the seed data stay local until the suite is frozen and
  the exposure decision is logged, as Frontier v5 does.
- Vision runs are expensive. Every sweep that spends quota is approved by
  the owner before launch and runs through the gate runner, one task at a
  time.
- Nothing from a run of any lab's product is used to train anything; traces
  carry the canary string and the suite declines to host trajectory
  datasets.

## Open questions (resolve before the first candidate)

1. Display stack: settled in milestone 1. Harbor's `computer-1` agent execs
   Xvfb, a window manager and Chromium inside the task container itself, so
   the suite's base image (`sandbox/Dockerfile.computer-use`) carries that
   stack and `sandbox/computer-use/ensure-display.sh` brings it up for runs
   with no agent (oracle, nop, cheat probes).
2. Which web apps to fork first: the shortlist needs permissive licenses,
   SQLite or single-file state for cheap checkers, and a UI worth reskinning.
3. Whether Claude in Chrome can be driven headlessly for a sweep, or whether
   the product rows are measured by hand at small n and footnoted as such.
4. How the accessibility tree is produced uniformly for native apps (AT-SPI)
   and browsers (CDP), so the `tree` conditions compare like with like.
5. Whether the dense reward is a checklist (ordered) or a set (unordered),
   since the choice shapes what an RL loop learns.
