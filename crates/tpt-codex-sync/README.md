# tpt-codex-sync

WebSocket transport for tpt-codex replicas, and the relay server they talk to.

## Run the relay

```text
cargo run -p tpt-codex-sync --bin tpt-codex-relay
```

By default it listens on `127.0.0.1:8787`. Options:

| Flag | Default | Meaning |
|------|---------|---------|
| `--addr ADDR` | `127.0.0.1:8787` | Address to listen on. `--addr=ADDR` also works. |
| `--max-message-bytes N` | `1048576` (1 MiB) | Largest WebSocket message accepted. A larger message closes that connection. |
| `--max-ops-per-doc N` | `100000` | Operations stored per document. Further operations are refused. |
| `ADDR` | | The same as `--addr`, for scripts written against the older positional form. |

Run `tpt-codex-relay --help` for the usage text. Flags must be positive whole numbers where they
take a number.

The relay stops accepting new connections when its serving loop is asked to stop
(`tpt_codex_sync::serve_until`). The binary does not yet wire that to ctrl-c: stop it by ending the
process. See the open items below.

## How the limits behave

* **Message size.** A WebSocket message or frame above `--max-message-bytes` closes that
  connection. The relay does not parse it first.
* **Operations per document.** Once a document holds `--max-ops-per-doc` operations, the relay
  refuses every further operation, including one a replica resends. The sender gets
  `{"error":"document is full"}` and nobody else does. The operation is not stored or forwarded.
  Presence and acknowledgements are not counted. `WsTransport::relay_error()` reports the refusal
  to a client. A replica keeps refused operations in its outbox.
* **Connections.** There is no limit on the number of connections. Each connection is a thread.

## Security

* **No authentication.** Any client that can reach the port can join any document by name and read
  or write its operations.
* **No encryption.** It speaks plain `ws://`.
* **Default bind address is loopback.** Keep it that way unless the network is trusted.
* **Not for public exposure.** Before exposing it beyond the local machine, put it behind TLS
  (for example a reverse proxy that terminates `wss://`) and an access check that identifies
  each client and decides which documents it may join. Limit the connection count at the proxy,
  since the relay does not.

## Open items

* ctrl-c shutdown: needs a signal handler. The workspace forbids `unsafe_code`, and no signal
  crate is in the lockfile, so this needs either an approved dependency (such as `ctrlc`) or an
  exception to that lint.
* Connection limits and authentication: not implemented.
