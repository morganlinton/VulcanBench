# VulcanBench ComputerUse macOS v1: the macOS computer-use suite (charter draft)

Status: **draft, 2026-11-11; milestone 1 (infrastructure) built; milestone 2
built four validated pilot tasks and ran the pilot on 2026-11-12 (results in
docs/results/computer-use-macos-v1-pilot/).** The owner asked for a computer-use suite focused
on macOS after the Linux suite (`tasks/computer-use-v1`, see
[COMPUTER_USE_CHARTER.md](COMPUTER_USE_CHARTER.md)) saturated: both reference
rows solved every capability task at n=1 (PR 293). The Linux suite stays as
it is. This charter reuses its rules wherever they carry over and records
what is different on macOS.

## Goal

Measure how well an agent operates a real Mac from the screen alone: native
Apple apps, the system's own conventions (the menu bar, sheets, dialogs,
System Settings, Finder), and workflows that cross apps, graded by the end
state each app leaves on disk. The suite should separate frontier agents
from day one: every capability task carries a reason a strong agent can fail.

## What carries over from the Linux charter

- Goals are phrased by outcome, never by widget; distractors and
  irreversible controls are present.
- Grading is by end state, with sub-metrics so a failure is attributed, and
  a `ui_path` sub-metric that requires evidence the agent used the screen.
- The validation pipeline: the untouched start state scores 0, the gold
  solution scores 1 three times, and at least two scripted shortcuts each
  score 0 with the named sub-metric catching them
  (`scripts/computer-use/validate_task.py`, which detects macOS tasks).
- The safety track, reported separately and never folded into pass@1
  (`cu_check.emit_safety` with `bad=` naming the dangerous sub-metric).
- Admission: n=3 against two reference rows under the `pixels` condition;
  reject tasks both references ace unless kept as deliberate anchors; the
  target keeps "at least one task both references fail".
- Engine and results in separate PRs; no published numbers from a dirty
  checkout.

## Environment (decided 2026-11-11, docs/DECISIONS.md)

- **One Tart VM per trial**, a copy-on-write clone of the task's seeded image,
  on Apple hardware (Apple's license allows two macOS VMs per Mac; sweeps run
  one trial at a time). macOS 27.2, from Cirrus Labs'
  `macos-golden-gate-vanilla` image pinned by digest.
- **Screen and input** through Virtualization.framework's own VNC server: the
  agent's clicks and keys arrive as hardware input and screenshots come from
  the VM's framebuffer, so the guest grants no Screen Recording or
  Accessibility permission. The display is 1280x800 points (a 2560x1600
  Retina framebuffer); the `pixels` condition shows the agent 1280x800.
- **The agent and its tools** run in harbor's ordinary container
  (`vulcanbench/sandbox:cu-mac`): the same MCP computer tools as the Linux
  suite (`vb_computer_mcp.py --backend vnc`), with Mac key names.
- **Grading** reads the stopped VM's disk: the collect hook shuts the guest
  down through System Events (apps quit and save), mounts the disk read-only
  on the host and copies the paths the task names. Checkers use
  `sandbox/computer-use/mac_state.py`.
- **Images are a recipe**, not a published artifact:
  `scripts/computer-use-macos/build_base.py` (guest agent, Automation grants
  for seeding, a deterministic desktop, an ordinary identity, a password the
  agent is never told) and `build_task_image.py` (frozen clock, seeded data,
  no network, no passwordless sudo).

## What the agent can and cannot do

- It sees the screen and acts with screenshot, click, double and triple
  click, drag, scroll, type and key. Typed text is US-keyboard ASCII (the VNC
  server maps key codes); tasks do not require other characters. The VZ VNC
  server carries no scroll-wheel events, so scrolling is posted inside the
  guest (`vbscroll`, through the environment's per-trial endpoint); the
  agent sees no difference.
- The guest has no network. Safari opens but loads nothing remote.
- Terminal is part of macOS and stays installed. The agent cannot become
  root (no passwordless sudo, an unknown password), and a checker requires
  app-consistent end state, so writing an app's store from Terminal is a
  standard cheat probe that must score 0.

## Families

- **A, one native app in depth**: Notes, Reminders, Calendar, Contacts,
  Preview, TextEdit.
- **B, the system itself**: Finder conventions, System Settings, the menu bar,
  sheets and dialogs, Spotlight, keyboard shortcuts, windows and Spaces.
- **C, cross-app workflows**: state carried between apps over tens of actions.

## Difficulty from day one

Every capability task carries at least one of: a long horizon (60 or more
actions), state carried across apps, dense perception (small text, many
similar rows, dates and sizes read from list views), a mid-task change, or a
macOS-specific interaction (sheets, contextual menus, drag between apps,
modifier-click selection, type-to-select).

## Safety on macOS

- Destructive near-misses: Empty Trash and Delete Immediately while the Trash
  already holds unrelated files; erasing or replacing instead of moving.
- Injection inside content the agent must read (a PDF in Preview, a note).
- System prompts the agent must not satisfy or guess through: an
  administrator password dialog, a permission request. The agent is never
  given the password.

## Scope

Built-in Apple apps only (no Apple Account, no App Store). Mail, Messages and
anything needing an account are out of scope for v1. The suite measures
agents through the MCP computer tools; native computer-use products (a
desktop agent running inside the VM) are a later row.

## Open questions

- Boot to a drawn desktop takes 30 to 50 seconds per trial; suspending a
  booted image and resuming clones may cut it.
- The VZ VNC server has crashed the VM when a client connected while a
  macOS security prompt was on screen, or seconds after another client
  disconnected. Trials now have one VNC client, connected once; a VM that
  dies mid-trial is an infrastructure failure, retried, never a result.
- Desktop widgets flash as placeholders for a few seconds at some boots
  although the base image hides them; agents connect after a 25 s settle.
