# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace also has a root
`CHANGELOG.md` that covers the whole project.

## [Unreleased]

### Added

- `WsTransport`, a WebSocket client that implements the core's `Transport` trait. `connect` joins a
  document by name and waits until the relay has subscribed the client.
- The relay: `serve`, `serve_with`, `serve_until`, `spawn` and `spawn_with`, and the
  `tpt-codex-relay` binary. A client joining a document gets its logged operations replayed, and
  then sees each other client's operations and presence.
- Presence and acknowledgements are forwarded to the other clients of a document but are not logged,
  so a client that joins later does not receive them.
- Relay limits: `--max-message-bytes` (a larger message closes the connection) and
  `--max-ops-per-doc` (further operations are refused, and the sender is told `document is full`).
  `Limits` holds both, and `WsTransport::relay_error()` reports a refusal.
- Command-line options `--addr`, `--max-message-bytes` and `--max-ops-per-doc` (each as `--flag VALUE`
  or `--flag=VALUE`), `--help`, and the positional address form for older scripts.
- `serve_until`, which stops accepting connections once a stop flag is set.
