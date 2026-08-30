# Replace the retired ShedCore controller; service rotas and reckonings no longer reconcile

We are decommissioning the legacy ShedCore controller that runs our
neighborhood tool-lending shed's service rota (a compiled binary whose
source left with the volunteer who wrote it) and replacing it with
`shedcore.py`. The Python rewrite was done from `docs/SPEC.md` and looks
correct on the happy path: registrations, enrollments, loans, and
returns all reply identically. But the shed desk console and the
seasonal reconcilers, all built against the old controller's actual
behavior over many years, keep flagging mismatches, and every mismatch
is about the rota and the counts, never about a printed figure:

- Rota order disagrees whenever the shelf mixes grades: replaying the
  same afternoon through both, the old controller's `S` lines come out
  in a different order than the rewrite's, with higher-grade tools
  sitting higher on the old rota than the ledger arithmetic says they
  should. On a shelf where every tool has the same grade the two rotas
  usually agree.
- Tools still out on loan make the old controller's rota: the desk
  crew is used to seeing a tool called in for service while a member
  still has it, and the `SEND` counts reflect that. The rewrite never
  lists a tool before it comes back, so its `SEND` counts run low on
  busy days.
- Rotas run on a quiet board disagree in shape: the old controller
  prints `SEND 0` when nothing is owed; the rewrite prints nothing at
  all. The trailer counts the service either way.
- Ties come off the rota in a different order after busy stretches:
  when tools are level on the books, the old controller's ordering
  depends on how the afternoon went, and it changes again after a
  monthly reckoning; the rewrite always uses registration order and
  never changes.
- Members who cart off armloads skew everything afterward: on days
  when one member had several tools out at the same time, the tools
  that member later returns ride higher on the old rota than the
  rewrite puts them, and the effect never wears off for that member.
- Veteran tools sink: tools that have been through service many times
  drift lower on the old controller's rota than the rewrite predicts,
  and sometimes stop showing up on it entirely while the rewrite still
  lists them.
- `MOK` counts drift apart over a season: the first monthly reckoning
  usually agrees, but by the second or third the old controller keeps
  reporting tools owed service that the rewrite says are clear, and
  now and then the other way around. After a reckoning the two rotas
  can disagree even on shelves that agreed before it.

The spec's own header warns about this: the document has drifted, and
**the controller's behavior is the contract**, not the spec. The retired
binary is available in `legacy/` for reference while you work
(`legacy/run < commands.txt`, one session per process, state accumulates
within a session). It is NOT present in production, so the replacement
must reproduce its behavior, not invoke it.

Make `shedcore.py` a drop-in behavioral replacement for the controller:
byte-for-byte identical reply lines (including rota order and
membership, `SEND` and `MOK` counts, error codes, and the trailer) for
any command stream, across registration, enrollment, loans, returns,
service rotas, and monthly reckonings, matching how the controller
actually validates, accrues, orders, services, reckons, and counts,
wherever that differs from what the spec says. Where the spec IS
accurate, nothing may change.
