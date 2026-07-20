# Security Policy

## Reporting vulnerabilities

vyre is the conformance prover for Santh's security infrastructure. A silent-wrong vulnerability in vyre could affect every downstream tool in the Santh ecosystem at internet scale.

If you discover a security vulnerability:

1. DO NOT file a public GitHub issue.
2. Email: security@santh.dev (PGP key below).
3. Expect initial response within 72 hours.
4. Expect disclosure coordination per the 90-day policy.

## Supported versions

| Version | Supported |
|---|---|
| 0.4.x alpha | yes (all patches receive security fixes) |
| 0.3.x and earlier | no |

## Classes of vulnerability (for triage)

### Critical (24-hour acknowledgment)
- False pass in certify()  -  a backend that should fail passes.
- Source code injection via TOML rule parser.
- Sandbox escape in reference interpreter.
- Any undefined behavior in the IR that allows arbitrary memory access via a published op.

### High (72-hour acknowledgment)
- Missing conformance check that lets a known-bad backend pass.
- Resource exhaustion in the conformance suite (single input causing OOM/DoS).
- Cat B tripwire bypass (a forbidden pattern escapes detection).

### Medium
- Documentation that instructs readers into insecure patterns.
- Dependency with a known CVE (updated per Cargo.lock schedule).

## Disclosure timeline

- Day 0: report received, triage begins.
- Day 3: initial classification shared with reporter.
- Day 14: fix in progress, CVE requested if applicable.
- Day 90: public disclosure + patch release (coordinated with reporter).

If the reporter wants to disclose earlier, we can coordinate.

## Acknowledgments

Reporters are credited in CHANGELOG.md and the GitHub security advisory unless they request anonymity.

## PGP key

(Placeholder  -  will be added before 1.0 release. For 0.4 alpha, use plain email.)
