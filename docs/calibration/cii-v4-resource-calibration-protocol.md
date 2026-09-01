# CII v4 resource calibration protocol (pre-registered)

Registered 2026-09-01, before any calibration run was started. Method
follows Anthropic's "Quantifying infrastructure noise in agentic coding
evals" as adopted by Terminal-Bench 4.0: pick a floor/ceiling resource
band empirically, by showing that scores at the tight end and the roomy
end of the band fall within noise of each other, and publish the band
with the results.

## What runs

Model: `codex:gpt-5.6-sol`, Codex CLI default reasoning effort, matching
every prior measurement in the program. Execution: agent-in-container
(`--agent-container --sandbox docker`), the standardized mode for suite
version 3.0. Tasks: all 23 tasks of `coding-intelligence-index-v4`,
repeat 1 per configuration, `--no-judges`. Runs are stored under
`runs-calibration/<config>/` so they never mix with the leaderboard run
pool.

## Configurations

| config  | mem floor | mem ceiling | cpu floor | cpu ceiling | pids |
|---------|-----------|-------------|-----------|-------------|------|
| tight   | none      | 1g          | none      | 1.0         | 512  |
| default | none      | 2g          | none      | 2.0         | 512  |
| roomy   | 2g        | 8g          | 2.0       | 4.0         | 1024 |

"default" is the band every run has used to date. "tight" halves it;
"roomy" quadruples the ceilings and adds guaranteed floors.

## Metrics, in order of importance

1. Infrastructure error rate per configuration: OOM kills (agent-phase
   `oom_kills` in the manifest plus verifier-phase infrastructure
   retries) and any resource-attributable run failure.
2. Score movement: mean functional and pass@1 per configuration.
3. Duration movement: per-task duration ratios across configurations,
   because CII v4's headline metrics are time-denominated.

## Decision rule (committed in advance)

- If tight and roomy scores are within one combined standard error and
  infrastructure errors are zero or attributable-and-retried at every
  configuration, the published band for version 3.0 is the default
  configuration's values, disclosed in the methodology.
- If tight shows infrastructure errors or a score deficit beyond noise,
  the published band moves up to the smallest configuration that is
  within noise of roomy.
- If durations shift by more than 25 percent in the median between any
  two configurations, time-denominated claims must state the band they
  were measured under, and the reference columns are rerun only under
  the published band.

## Caveats stated in advance

- n=1 per task per configuration resolves large effects only; the
  known run-to-run functional variance of this model on these tasks
  (for example granarycore 0.143 to 0.786 at fixed resources) means
  small per-task differences are not interpretable. Aggregates and
  infrastructure error counts are the signal.
- The agent image is amd64 running emulated on the measurement host; a
  two-run spot check on granarycore showed container durations inside
  the host-run spread, so emulation is not treated as a confound for
  this workload, but per-run durations remain noisier than scores.
