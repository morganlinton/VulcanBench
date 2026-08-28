# Replace the retired SyncPeer daemon; replication partners refuse to certify the rewrite

We are decommissioning the SyncPeer replication daemon (a compiled
artifact whose source predates the 2016 rewrite of everything else) and
replacing it with `syncpeer.py`. The rewrite follows `docs/SPEC.md` and
passes casual testing, but partner certification, which replays recorded
wire sessions byte-for-byte, fails on nearly every transcript: negotiated
versions come back wrong for some clients, session tokens do not behave
the way partners expect across reconnects, key listings arrive in the
wrong order and match keys they should not, overwrites answer
differently, long values come back altered, and the connection-close
accounting never matches.

The spec's own header says it has drifted: **the peer's wire behavior is
the contract**, because every replication partner was certified against
the daemon itself. The retired binary is in `legacy/` for reference while
you work (`legacy/run` starts it and prints its port; talk to it over
TCP). It is NOT present in production, so the replacement must reproduce
its behavior, not proxy to it.

Make `syncpeer.py` a drop-in wire-level replacement: byte-for-byte
identical reply lines for any sequence of connections and commands,
matching how the daemon actually negotiates, identifies sessions, stores,
lists, and counts, wherever that differs from the spec. Where the spec IS
accurate, nothing may change.
