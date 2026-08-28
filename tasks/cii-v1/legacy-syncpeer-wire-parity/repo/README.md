# syncpeer

Python replacement for the retired SyncPeer replication peer.

- `syncpeer.py`: the replacement peer (`python syncpeer.py --port 0`
  prints `LISTENING <port>` and serves one client at a time; the store
  lives for the process).
- `docs/SPEC.md`: the written protocol spec. **Read its drift warning**:
  the spec is stale in places, and every peer certified downstream was
  built against the engine's wire behavior, which is the contract.
- `legacy/`: the retired peer binary, for reference while developing
  (`legacy/run`, then talk to the printed port). It is not present in
  production.
