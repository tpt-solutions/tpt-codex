# tpt-codex-sync

WebSocket transport for tpt-codex replicas, and the relay server they talk to. The crate has two
parts: `WsTransport`, a client that implements the core's `Transport` trait, and a relay that keeps
each document's operations in memory and forwards new messages to the other clients of that
document. The relay ships as the `tpt-codex-relay` binary and can also run inside another program.

## Status

Pre-release: version `0.1.0-spec`, `publish = false`. The relay has no authentication and no
encryption, so it is for trusted local networks and development (see [Security](#security)).

## What it does, and what it does not

It does:

* Joins a client to a document by name, replays the document's operations to a new client, and
  forwards each new message to the other clients of that document.
* Keeps each document's operations in memory, de-duplicated by the core's `Relay`.
* Caps message size and operations per document (see [How the limits behave](#how-the-limits-behave)).
* Stops accepting connections on request when run as a library (`serve_until`).

It deliberately does not:

* Authenticate clients, encrypt traffic, or check which documents a client may join. It speaks
  plain `ws://`.
* Store anything on disk. Restarting the relay forgets every document.
* Limit the number of connections, or rate-limit clients. Each connection gets a thread.
* Keep presence or acknowledgements. Both are forwarded to the current clients and are not
  replayed to a client that joins later.
* Interpret the operations. It logs them and forwards them; the replicas apply them.

## Where it sits in the workspace

* Depends on `tpt-codex-core` (`Message`, `Relay`, `Transport`, `Publish`), `serde`, `serde_json`
  and `tungstenite` 0.24.
* Used by `tpt-codex-app`, as a dev-dependency only. Its presence test talks through a real relay.
  The app's wasm build does not include this crate.
* The browser demo (`tpt-codex-web`) does not link this crate. Its page opens a plain WebSocket to
  the relay from JavaScript and joins for presence only.

## Wire protocol

1. The client opens a WebSocket to the relay and sends one text frame, `{"join":"<document>"}`.
2. The relay subscribes the client, then replies with the text frame `{"joined":true}`.
3. The relay replays the document's logged operations to the client, one frame each.
4. From then on, each text frame is the JSON form of a `tpt_codex_core::Message`: an operation,
   a presence update or an acknowledgement. The relay logs operations only. It forwards each
   message to the other clients of the document, and never echoes a message back to its sender.

When the relay refuses an operation, it sends the sender the text frame `{"error":"document is full"}`.
`WsTransport` ignores any other frame that is not a message.

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
take a number. An unknown option, a missing value or a second address exits with status 2. A
failure to bind the address exits with status 1.

Stop the relay with ctrl-c, which ends the process. Nothing needs saving: the relay keeps its
operations in memory only. Embedders that want a graceful stop can use `tpt_codex_sync::serve_until`.

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

## Features and key public API

| Item | What it is |
|------|------------|
| `WsTransport::connect(addr, document)` | Connects to a relay and joins a document. Returns once the relay has subscribed the client, so nothing sent afterwards can be missed. |
| `WsTransport` (`Transport` impl) | `send` writes a message as a JSON text frame. `receive` returns the next message, or `None` when none has arrived or the connection has closed. |
| `WsTransport::error()`, `relay_error()` | The reason the connection closed, and the most recent refusal from the relay. |
| `serve(listener)`, `serve_with(listener, limits)` | Serve connections on a bound listener until it fails. |
| `serve_until(listener, limits, stop)` | As `serve_with`, but returns `Ok` once the `AtomicBool` `stop` is set. Open connections are not closed. |
| `spawn(addr)`, `spawn_with(addr, limits)` | Bind an address and serve it on a background thread. Returns the bound address, which is useful with port `0`. |
| `Limits` | `max_message_bytes` and `max_ops_per_doc`. `Limits::default()` uses the defaults in the table above. |
| `Config`, `parse_args`, `USAGE` | The command-line parser and usage text the binary uses. |
| `DEFAULT_ADDR`, `DEFAULT_MAX_MESSAGE_BYTES`, `DEFAULT_MAX_OPS_PER_DOC` | The defaults. |

A replica connected through this crate:

```rust,ignore
use tpt_codex_core::{Replica, Transport};
use tpt_codex_sync::{WsTransport, spawn};

let addr = spawn("127.0.0.1:0")?; // an embedded relay, for tests or a local demo
let mut transport = WsTransport::connect(&addr.to_string(), "notes")?;
let replica = Replica::new(1);
// Send the replica's outgoing messages with `transport.send(&message)`, and
// feed `transport.receive()` into the replica as they arrive.
```

## Errors

* `WsTransport::connect` returns an `io::Error`. It fails with `TimedOut` if the relay does not
  confirm the join within ten seconds.
* `Transport::send` returns `Err(String)` when the connection has failed, and the same error is
  kept in `error()`. `receive` returns `None` both when nothing has arrived and when the
  connection has closed; check `is_connected()` or `error()` to tell them apart.
* On the relay, a connection whose first frame is not a join, or that fails, ends on its own.
  Other connections are not affected.

## Platform

This crate is `std` only and uses threads and blocking sockets. It is not built for wasm. The
browser side of the protocol lives in `tpt-codex-web`.

## Testing

```text
cargo test -p tpt-codex-sync
```

| Test file | What it covers |
|-----------|----------------|
| `tests/relay.rs` | Two replicas converging over a real relay, a late joiner catching up from the log, documents kept apart, presence delivered but not replayed, and a repeated send not forwarded twice. |
| `tests/collaboration.rs` | Several people editing text and grids together end with the same document. |
| `tests/presence.rs` | A cursor set on one session reaches the other, and only the newest cursor is sent on reconnect. |
| `tests/limits.rs` | The size cap closes an oversized connection, the per-document operation cap, that the cap is per document, and that `serve_until` stops accepting. |
| `tests/flags.rs` | The command-line parser: defaults, both flag forms, positional address, refused values, and `--help`. |

The browser presence check in `crates/tpt-codex-web/browser` runs the relay binary against two
demo pages. See that crate's README.

## Open items

* ctrl-c shutdown: needs a signal handler. The workspace forbids `unsafe_code`, and no signal
  crate is in the lockfile, so this needs either an approved dependency (such as `ctrlc`) or an
  exception to that lint.
* Connection limits and authentication: not implemented.

## License

Dual-licensed under MIT OR Apache-2.0. See the root [LICENSE-MIT](../../LICENSE-MIT) and
[LICENSE-APACHE](../../LICENSE-APACHE).
