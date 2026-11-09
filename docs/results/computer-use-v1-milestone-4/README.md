# ComputerUse v1, milestone 4: two long-horizon tasks and two perception conditions at n=3

Date: 2026-11-09. Suite `computer-use-v1` version 0.4.0 at repo commit
`51b9d010` (branch `computer-use/milestone-4`, PR 281), image
`vulcanbench/sandbox:cu-base`
`sha256:5457616784e6c9845ed4a77749a3519d36ee303876efe10401853ddfbe3ed737`,
harbor 0.24.0. The suite is not frozen (no `suite.lock.json`), so these
numbers are a gate reading on candidate tasks, not a published column, and
they rank nothing.

One reference row ran: `claude-code` (Claude Code 2.1.302, model
`claude-opus-5-5`, reasoning effort medium, on the owner's subscription),
acting only through the task's MCP computer tools (Bash, file, web and
sub-agent tools disallowed). Three attempts per task, one trial at a time,
launched by `scripts/computer-use/run_gate.sh` with `GATE_ATTEMPTS=3` and
`GATE_JOB_SUFFIX=-m4`. Four jobs ran back to back from 14:09 to 15:00 UTC:
the two tasks milestone 4 added, under the default perception condition,
and `ledger-reconcile` from milestone 3 twice more, under the two
non-default conditions milestone 4 made selectable.

Perception conditions, as the charter's table names them: `pixels+zoom` is
what every reading before this one was taken under (the MCP computer tools
have carried a zoom tool since milestone 1); `pixels` removes the zoom
tool; `pixels-lowres` also scales every screenshot to 0.75 (768 by 675
pixels for the 1024 by 900 display) and maps the agent's coordinates back.
The gate sets a condition with `CU_GATE_CONDITION`, runs a copy of the task
whose MCP server starts with `--condition`, and records the condition in
the job name, the status file and the trials file. The agent streams
confirm it: the two `pixels` and `pixels-lowres` jobs contain no zoom call
(the milestone 3 trials made two or three each), and every screenshot the
`pixels-lowres` agent received measures 768 by 675.

Per-trial records, every sub-metric, token counts and durations are in
[trials.json](trials.json), built by `scripts/computer-use/summarize_gate.py`.

## Results

| Task | Family | Condition | Solved | Turns | Agent wall (s) | API time (s) | Cost (USD, list) |
|---|---|---|---|---|---|---|---|
| files-invoice-entry | B | pixels+zoom | 3 of 3 | 91, 64, 63 | 165, 128, 114 | 117, 93, 82 | 1.85, 1.30, 1.19 |
| relaydesk-sla-sweep | C | pixels+zoom | 2 of 3 | 90, 90, 102 | 307, 287, 398 | 152, 129, 139 | 1.97, 1.93, 2.12 |
| ledger-reconcile | C | pixels | 3 of 3 | 38, 34, 39 | 85, 96, 126 | 58, 52, 62 | 0.71, 0.62, 0.72 |
| ledger-reconcile | C | pixels-lowres | 3 of 3 | 39, 36, 44 | 110, 121, 143 | 80, 75, 94 | 0.56, 0.50, 0.66 |

Turns are Claude Code's `num_turns`. Agent wall is the time harbor spent
in the agent step; trial wall (container build, boot, verifier) is in the
JSON. Cost is Claude Code's list-price estimate from its result record; the
runs were on a subscription and nothing was billed per call. Across the 12
trials the estimate sums to 14.13 USD and 85,141 output tokens.

In every trial the changes came through the apps' own forms or the display:
all 12 trials score `ui_path` 1, every audited write in the two web apps
carries the form token and the UI marker, and the invoice trials show 600
to 659 real input events before the ledger was saved (the keyboard gold
needs 1,542). No trial failed a guard sub-metric; the one lost trial lost
exactly one sub-metric, described next.

## The failure

### relaydesk-sla-sweep, 2 of 3: seven escalated, six reported

The task: find every High ticket that is Open or Pending, was created
before Friday 27 February 09:15 and has no comment, across six pages of 55
tickets with seven near-misses; comment "SLA breached, escalated" on each,
set it to Urgent, and send Priya one mail listing exactly those tickets. A
new High ticket arrives 30 seconds in and must be left alone. Seven tickets
qualify.

All three attempts found the same seven tickets, commented on each and set
each to Urgent (`targets_comment`, `targets_priority` and
`targets_other_fields` 1, `new_comments` 7), touched none of the
near-misses (`near_misses_untouched` 1), left the arriving ticket alone and
named it in the final message, and sent one reply to Priya's mail from the
compose form. In the two solved attempts the reply listed the seven ids.

In the failed attempt the reply listed six: 5003, 5011, 5018, 5034, 5041
and 5052. The missing one, 5026, is the second ticket the agent escalated
(its comment is stamped 09:19; the last, on 5003, 09:29). The agent's own
progress notes show where it went. It worked down the list from the highest
number and kept a running tally: "I've now escalated 5052, 5026, 5041,
5018 and 5011. Next I'll check 5034." Two tickets later the tally was
rewritten in ascending order as "6 tickets: 5003, 5011, 5018, 5034, 5041
and 5052", and 5026 was gone. The note then said that a new ticket had
arrived and the list would be rechecked before the mail; the recheck did
not restore 5026, which by then carried the agent's own comment and so
looked like a ticket to skip. The final message says "Six tickets had
breached the SLA", lists the same six, and still cites 5026 by number in a
remark about how the tracker attributes comments. `mail_ids_exact` scored
0 and the task 0. Nothing in the task or the checker explains it: the mail
is graded on the set of ids it contains, and all seven escalations are in
the tracker's audit.

This is the first trial the suite has lost for a capability reason since
the task fixes of milestones 2 and 3, and it is the kind the long-horizon
tasks were built to find: not a misread figure or a missed arrival, but
state the agent has to carry correctly across about ninety turns and into
a second application. The mechanism is worth naming: once the agent has
acted on a ticket, the world no longer distinguishes that ticket from the
near-misses it was told to leave alone, so a re-read of the screen cannot
repair a tally that dropped it. One in three is a thin reading; the task
stays as it is and the next gate adds attempts.

## ledger-reconcile under the two other conditions

| Reading | Condition | Solved | Turns | Agent wall (s) | API time (s) | Cost (USD, list) | Cache-creation tokens |
|---|---|---|---|---|---|---|---|
| milestone 3, repo `8a5399fc` | pixels+zoom | 2 of 3 | 40, 40, 41 | 112, 113, 137 | 84, 84, 91 | 0.74, 0.74, 0.74 | 58,753, 61,205, 57,952 |
| this note | pixels | 3 of 3 | 38, 34, 39 | 85, 96, 126 | 58, 52, 62 | 0.71, 0.62, 0.72 | 58,760, 49,656, 56,848 |
| this note | pixels-lowres | 3 of 3 | 39, 36, 44 | 110, 121, 143 | 80, 75, 94 | 0.56, 0.50, 0.66 | 38,239, 35,368, 46,486 |

The first row is the milestone 3 note's reading, at suite 0.3.0 and before
the checker wording fix that its lost trial exposed; the rows are not a
controlled comparison. What the two new rows show on their own: with the
zoom tool removed, and again with the screenshots at three quarters size,
the reference read both dense tables correctly in every attempt and wrote
exactly the four discrepancies with correct details (`report_ids_exact`
and `report_details` 1 in all six trials), paging through the tracker's
Billing list five to seven times as before. Turn counts are the same as
under `pixels+zoom`. The smaller screenshots cost less: cache-creation
tokens, which carry the images, fall by about a third under
`pixels-lowres`, and the list-price estimate with them, while turn counts
and API time stay in the same range, because the agent takes the same
number of steps.

On this task, at this size, the reference does not need the zoom tool and
is not hurt by 0.75 scale. The charter's open question on perception
conditions is therefore not answered in the direction of "smaller text
breaks the reference" by a 1024 by 900 display scaled to 768 by 675; a
condition that separates will need either denser content or a stronger
reduction, and the suite now has the switch to try both without touching
the tasks.

## Observations

- `files-invoice-entry` went 3 of 3 in 114 to 165 seconds of agent wall,
  faster than the 240-second keyboard gold. Every attempt entered the five
  new invoices with the totals the ledger expects, skipped the one already
  on row 15, entered the re-sent Kestrel invoice once, and left the credit
  note as the only file in `incoming`. All three entered the dates as text
  to match the existing rows, which the shared ODS reader accepts alongside
  real date cells. All three flagged the Tidewater file, whose name says
  "statement" while the document is headed "Invoice", and entered it.
- All three invoice attempts also reported that the Kestrel invoice's
  anti-scratch coating line (40 at 9.62) comes to 384.80 while the line
  shows 385.00. That is a flaw in the seed, not a planted trap: the unit
  price is 9.625 and the PDF writer rounds it to two places. It did not
  affect grading (the stated total is what the ledger takes) but it should
  be fixed on the engine side so the invoices add up.
- Two sweep attempts reported that Relay Desk records a comment under the
  ticket's assignee where there is one, and as "Support" otherwise, instead
  of under the person using the app. That is a realism flaw in the shared
  tracker, like the Postbox clock noted in milestone 3, and is logged for
  a later fix; it does not touch grading.
- Every sweep attempt named the near-miss created at 09:40 on the Friday
  (25 minutes inside the cutoff) and left it alone, and every final message
  listed the other High tickets it had skipped with a reason; two of the
  three also called out the "Waiting on Finance" ticket as a status the
  instruction did not cover.
- The subscription's five-hour usage window went from 0.00 to 0.83 over
  the four jobs; a fifth job of this size would have been refused by the
  gate's preflight at the default ceiling.

## What this reading does and does not show

- The milestone's bar, at least one of the two new tasks below 3 of 3 for a
  capability reason, was met by `relaydesk-sla-sweep` at 2 of 3, with the
  lost trial traced to a bookkeeping error across a long horizon and not to
  a defect in the task. `files-invoice-entry` did not supply a hard tail.
- The perception-condition switch works end to end and is recorded with
  the numbers. On `ledger-reconcile` neither non-default condition cost the
  reference an attempt; the suite still has no condition shown to separate.
- n=3 on one reference row gives no interval worth printing and is not
  evidence about any model other than the one run. The native computer-1
  row has not run (it needs an API key the gate does not have).
