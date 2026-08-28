# SyncPeer wire protocol (spec v1.2, last updated 2016)

> Maintenance note (2021): this document has drifted from the production
> peer. Where they disagree, **the peer's wire behavior is the contract**;
> every replication partner was certified against the peer.

TCP, line-oriented (`\n`), one client at a time. The key-value store and
the session registry live for the whole process. The server prints
`LISTENING <port>` on stdout at startup.

## Handshake

First line of every connection: `HELLO <version> <node>`.
`version`: 1-9; the server supports protocol versions 1-3 and negotiates
`min(version, 3)`. `node`: 1-8 alphanumerics identifying the peer.
Reply: `WELCOME <negotiated> <sessionid>`, where the session id is an
opaque per-connection token. A malformed handshake answers
`ERR HANDSHAKE` and the connection closes.

## Commands

- `PUT <key> <value>` - store. `key`: 1-16 alphanumerics; `value`: 1-64
  characters, no spaces. Reply `OK` (or `ERR FULL` at 4096 keys).
- `GET <key>` - `VAL <value>` or `ERR NOTFOUND`.
- `DEL <key>` - `OK` or `ERR NOTFOUND`.
- `KEYS <prefix>` - every key starting with `<prefix>` (case-sensitive),
  in lexicographic order: `KEY <key>` lines, then `END <count>`.
- `BYE` - `GOODBYE <count>`, where `<count>` is the number of commands
  served after the handshake; the connection closes.

Malformed commands answer `ERR FMT`.
