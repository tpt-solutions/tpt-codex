// SPDX-License-Identifier: MIT OR Apache-2.0
//! WebSocket transport for replicas, and the relay server they talk to.
//!
//! A client connects, sends `{"join":"<document>"}`, and then exchanges [`Message`]s as JSON text
//! frames. The server replays the document's log to a new client, forwards each new message to
//! the other clients of that document, and keeps the log in a [`Relay`]. Presence is forwarded
//! but not logged.
//!
//! The relay caps what it accepts: a WebSocket message above [`Limits::max_message_bytes`] closes
//! the connection, and once a document holds [`Limits::max_ops_per_doc`] operations the relay
//! refuses further operations with `{"error":"document is full"}`, sent to the sender only. See
//! the crate README for the security notes.
//!
//! This is plain `ws://` with no authentication. Put it behind TLS and an access check before
//! exposing it.

use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tpt_codex_core::{Message, Publish, Relay, Transport};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message as Frame, WebSocket};

/// How long a socket waits for a frame before the loop checks its outbound queue.
const POLL: Duration = Duration::from_millis(5);

/// How long the opening handshake may take. The polling timeout is too short for it.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// How long the accept loop sleeps when no client is waiting, so it can check its stop flag.
const ACCEPT_POLL: Duration = Duration::from_millis(20);

/// The address the binary listens on when none is given.
pub const DEFAULT_ADDR: &str = "127.0.0.1:8787";

/// Largest WebSocket message the relay accepts, in bytes, unless `--max-message-bytes` says
/// otherwise. A single operation is far smaller than this.
pub const DEFAULT_MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// Most operations the relay stores for one document, unless `--max-ops-per-doc` says otherwise.
pub const DEFAULT_MAX_OPS_PER_DOC: usize = 100_000;

/// The limits a relay enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest accepted WebSocket message in bytes. A larger message closes the connection.
    pub max_message_bytes: usize,
    /// Most operations stored per document. Further operations are refused, not stored.
    pub max_ops_per_doc: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES,
            max_ops_per_doc: DEFAULT_MAX_OPS_PER_DOC,
        }
    }
}

/// The frame the relay sends to a sender whose operation it refuses.
const FULL_FRAME: &str = r#"{"error":"document is full"}"#;

/// A refusal frame from the relay.
#[derive(Deserialize)]
struct Refusal {
    error: String,
}

/// A client connection to a relay, for one document.
#[derive(Debug)]
pub struct WsTransport {
    socket: WebSocket<TcpStream>,
    error: Option<String>,
    relay_error: Option<String>,
}

impl WsTransport {
    /// Connects to the relay at `addr` (for example `127.0.0.1:8787`) and joins `document`.
    pub fn connect(addr: &str, document: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(HANDSHAKE))?;
        stream.set_nodelay(true)?;
        let (mut socket, _) = tungstenite::client(format!("ws://{addr}/"), stream)
            .map_err(|e| io::Error::other(e.to_string()))?;
        socket.get_mut().set_read_timeout(Some(POLL))?;
        let join = serde_json::json!({ "join": document }).to_string();
        socket
            .send(Frame::Text(join))
            .map_err(|e| io::Error::other(e.to_string()))?;
        // Wait until the relay has subscribed this client, so nothing sent after this returns
        // can be missed. Presence is not logged, so a missed update is lost for good.
        socket.get_mut().set_read_timeout(Some(HANDSHAKE))?;
        let deadline = std::time::Instant::now() + HANDSHAKE;
        loop {
            match socket.read() {
                Ok(Frame::Text(text)) if text.as_str() == JOINED => break,
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
                        && std::time::Instant::now() < deadline => {}
                Err(e) => return Err(io::Error::other(e.to_string())),
            }
            if std::time::Instant::now() >= deadline {
                return Err(io::Error::new(
                    ErrorKind::TimedOut,
                    "the relay did not join the document",
                ));
            }
        }
        socket.get_mut().set_read_timeout(Some(POLL))?;
        Ok(Self {
            socket,
            error: None,
            relay_error: None,
        })
    }

    /// The error that closed the connection, if it has closed.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The most recent refusal from the relay, such as `document is full`, if any. A refused
    /// operation is not stored, so the sender still has it and may retry or report it.
    pub fn relay_error(&self) -> Option<&str> {
        self.relay_error.as_deref()
    }
}

impl Transport for WsTransport {
    type Error = String;

    fn is_connected(&self) -> bool {
        self.error.is_none()
    }

    fn send(&mut self, message: &Message) -> Result<(), String> {
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        let text = serde_json::to_string(message).map_err(|e| e.to_string())?;
        self.socket
            .send(Frame::Text(text))
            .map_err(|e| e.to_string())
    }

    fn receive(&mut self) -> Option<Message> {
        if self.error.is_some() {
            return None;
        }
        loop {
            match self.socket.read() {
                Ok(Frame::Text(text)) => {
                    if let Ok(message) = serde_json::from_str::<Message>(text.as_str()) {
                        return Some(message);
                    }
                    if let Ok(refusal) = serde_json::from_str::<Refusal>(text.as_str()) {
                        self.relay_error = Some(refusal.error);
                    }
                    // Any other frame is ignored.
                }
                Ok(Frame::Close(_)) => {
                    self.error = Some("closed by the relay".into());
                    return None;
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    return None;
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    return None;
                }
            }
        }
    }
}

/// The frame the relay sends once it has subscribed a client to the document.
const JOINED: &str = r#"{"joined":true}"#;

/// The first frame a client sends.
#[derive(Deserialize)]
struct Join {
    join: String,
}

/// Shared state: the logs, and the queues of the connections subscribed to each document.
#[derive(Default)]
struct Hub {
    relay: Relay,
    next_id: u64,
    subscribers: BTreeMap<String, Vec<(u64, Sender<String>)>>,
}

/// Serves relay connections on `listener` with the default [`Limits`], one thread per
/// connection. Runs until the listener fails.
pub fn serve(listener: TcpListener) -> io::Result<()> {
    serve_with(listener, Limits::default())
}

/// Serves relay connections on `listener` with `limits`. Runs until the listener fails.
pub fn serve_with(listener: TcpListener, limits: Limits) -> io::Result<()> {
    serve_until(listener, limits, Arc::new(AtomicBool::new(false)))
}

/// Serves relay connections on `listener` until `stop` is set. The listener stops accepting
/// once `stop` is set and this returns `Ok`. Connections already open are not closed here: they
/// end when their clients close them or when the process exits.
pub fn serve_until(listener: TcpListener, limits: Limits, stop: Arc<AtomicBool>) -> io::Result<()> {
    listener.set_nonblocking(true)?;
    let hub = Arc::new(Mutex::new(Hub::default()));
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                // Accepted sockets may inherit the listener's non-blocking mode on some systems.
                stream.set_nonblocking(false)?;
                let hub = Arc::clone(&hub);
                thread::spawn(move || {
                    // A connection that fails is only that connection. The others carry on.
                    let _ = handle(stream, hub, limits);
                });
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => thread::sleep(ACCEPT_POLL),
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::ConnectionAborted | ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Binds `addr` and serves it with the default [`Limits`]. Returns the bound address, which is
/// useful when `addr` ends in `:0`. Serving happens on a background thread.
pub fn spawn(addr: &str) -> io::Result<SocketAddr> {
    spawn_with(addr, Limits::default())
}

/// Binds `addr` and serves it with `limits`, on a background thread.
pub fn spawn_with(addr: &str, limits: Limits) -> io::Result<SocketAddr> {
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;
    thread::spawn(move || {
        let _ = serve_with(listener, limits);
    });
    Ok(local)
}

/// Command-line settings for the `tpt-codex-relay` binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The address to listen on.
    pub addr: String,
    /// The limits to enforce.
    pub limits: Limits,
}

/// The usage text for the `tpt-codex-relay` binary.
pub const USAGE: &str = "usage: tpt-codex-relay [--addr ADDR] [--max-message-bytes N] \
[--max-ops-per-doc N] [ADDR]

  --addr ADDR             address to listen on (default 127.0.0.1:8787)
  --max-message-bytes N   largest WebSocket message accepted (default 1048576)
  --max-ops-per-doc N     operations stored per document (default 100000)
  ADDR                    the same as --addr, for compatibility with older scripts";

/// Parses the arguments after the program name. Accepts `--addr ADDR` or `--addr=ADDR`, the two
/// numeric limits in the same two forms, a positional address, and `-h` or `--help`, which
/// returns [`USAGE`] as the error.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Config, String> {
    let mut config = Config {
        addr: DEFAULT_ADDR.to_string(),
        limits: Limits::default(),
    };
    let mut positional = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "-h" || arg == "--help" {
            return Err(USAGE.to_string());
        }
        if !arg.starts_with('-') {
            if positional {
                return Err(format!("more than one address given\n{USAGE}"));
            }
            positional = true;
            config.addr = non_empty(arg)?;
            continue;
        }
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) => (name.to_string(), Some(value.to_string())),
            None => (arg.clone(), None),
        };
        let mut value = |name: &str| -> Result<String, String> {
            match inline.clone().or_else(|| args.next()) {
                Some(value) => Ok(value),
                None => Err(format!("{name} needs a value\n{USAGE}")),
            }
        };
        match name.as_str() {
            "--addr" => config.addr = non_empty(value("--addr")?)?,
            "--max-message-bytes" => {
                config.limits.max_message_bytes = positive(&value("--max-message-bytes")?)?;
            }
            "--max-ops-per-doc" => {
                config.limits.max_ops_per_doc = positive(&value("--max-ops-per-doc")?)?;
            }
            _ => return Err(format!("unknown option {name}\n{USAGE}")),
        }
    }
    Ok(config)
}

fn non_empty(value: String) -> Result<String, String> {
    if value.trim().is_empty() {
        Err(format!("the address is empty\n{USAGE}"))
    } else {
        Ok(value)
    }
}

fn positive(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!(
            "expected a positive whole number, got {value:?}\n{USAGE}"
        )),
    }
}

fn handle(
    stream: TcpStream,
    hub: Arc<Mutex<Hub>>,
    limits: Limits,
) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(HANDSHAKE))?;
    stream.set_nodelay(true)?;
    let config = WebSocketConfig {
        max_message_size: Some(limits.max_message_bytes),
        max_frame_size: Some(limits.max_message_bytes),
        ..WebSocketConfig::default()
    };
    let mut socket = tungstenite::accept_with_config(stream, Some(config))?;
    socket.get_mut().set_read_timeout(Some(POLL))?;

    // Wait for the join frame.
    let document = loop {
        match socket.read() {
            Ok(Frame::Text(text)) => {
                let join: Join = serde_json::from_str(text.as_str())?;
                break join.join;
            }
            Ok(Frame::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e.into()),
        }
    };

    // Subscribe, and collect the log to replay to this client. The join is acknowledged first.
    let (tx, rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let (id, history) = {
        let mut hub = hub.lock().map_err(|_| "relay state poisoned")?;
        hub.next_id += 1;
        let id = hub.next_id;
        hub.subscribers
            .entry(document.clone())
            .or_default()
            .push((id, tx));
        let history: Vec<Message> = hub.relay.since(&document, 0).to_vec();
        (id, history)
    };
    socket.send(Frame::Text(JOINED.into()))?;
    for message in &history {
        socket.send(Frame::Text(serde_json::to_string(message)?))?;
    }

    let result = pump(&mut socket, &hub, &document, id, &rx, limits);

    // Unsubscribe whatever the reason the connection ended.
    if let Ok(mut hub) = hub.lock() {
        if let Some(subs) = hub.subscribers.get_mut(&document) {
            subs.retain(|(sub, _)| *sub != id);
        }
    }
    result
}

/// Reads frames from the client and writes queued frames to it, until the connection ends.
fn pump(
    socket: &mut WebSocket<TcpStream>,
    hub: &Arc<Mutex<Hub>>,
    document: &str,
    id: u64,
    rx: &Receiver<String>,
    limits: Limits,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match socket.read() {
            Ok(Frame::Text(text)) => {
                if let Ok(message) = serde_json::from_str::<Message>(text.as_str())
                    && relay_message(hub, document, id, message, limits)? == Accepted::Refused
                {
                    socket.send(Frame::Text(FULL_FRAME.into()))?;
                }
            }
            Ok(Frame::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e.into()),
        }
        while let Ok(text) = rx.try_recv() {
            socket.send(Frame::Text(text))?;
        }
    }
}

/// Whether the relay took a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accepted {
    Yes,
    Refused,
}

/// Logs `message` and forwards it to the other clients of `document`. A repeated operation is
/// dropped without being forwarded again. An operation is refused, and not forwarded, when the
/// document already holds `limits.max_ops_per_doc` operations. Presence is not counted.
fn relay_message(
    hub: &Arc<Mutex<Hub>>,
    document: &str,
    from: u64,
    message: Message,
    limits: Limits,
) -> Result<Accepted, Box<dyn std::error::Error>> {
    let mut hub = hub.lock().map_err(|_| "relay state poisoned")?;
    let is_op = matches!(message, Message::Op(_));
    if is_op && hub.relay.len(document) >= limits.max_ops_per_doc {
        return Ok(Accepted::Refused);
    }
    let outcome = hub.relay.publish(document, message.clone());
    if outcome == Publish::Duplicate {
        return Ok(Accepted::Yes);
    }
    let text = serde_json::to_string(&message)?;
    if let Some(subs) = hub.subscribers.get(document) {
        for (sub, tx) in subs {
            if *sub != from {
                // A closed queue belongs to a connection that is already going away.
                let _ = tx.send(text.clone());
            }
        }
    }
    Ok(Accepted::Yes)
}
