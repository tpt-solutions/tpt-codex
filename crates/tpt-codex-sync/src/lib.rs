// SPDX-License-Identifier: MIT OR Apache-2.0
//! WebSocket transport for replicas, and the relay server they talk to.
//!
//! A client connects, sends `{"join":"<document>"}`, and then exchanges [`Message`]s as JSON text
//! frames. The server replays the document's log to a new client, forwards each new message to
//! the other clients of that document, and keeps the log in a [`Relay`]. Presence is forwarded
//! but not logged.
//!
//! This is plain `ws://` with no authentication. Put it behind TLS and an access check before
//! exposing it.

use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tpt_codex_core::{Message, Publish, Relay, Transport};
use tungstenite::{Message as Frame, WebSocket};

/// How long a socket waits for a frame before the loop checks its outbound queue.
const POLL: Duration = Duration::from_millis(5);

/// How long the opening handshake may take. The polling timeout is too short for it.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// A client connection to a relay, for one document.
#[derive(Debug)]
pub struct WsTransport {
    socket: WebSocket<TcpStream>,
    error: Option<String>,
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
        })
    }

    /// The error that closed the connection, if it has closed.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
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
                    // A frame that is not a message is ignored.
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

/// Serves relay connections on `listener`, one thread per connection. Runs until the listener
/// fails.
pub fn serve(listener: TcpListener) -> io::Result<()> {
    let hub = Arc::new(Mutex::new(Hub::default()));
    for stream in listener.incoming() {
        let stream = stream?;
        let hub = Arc::clone(&hub);
        thread::spawn(move || {
            // A connection that fails is only that connection. The others carry on.
            let _ = handle(stream, hub);
        });
    }
    Ok(())
}

/// Binds `addr` and serves it. Returns the bound address when `addr` ends in `:0`, by serving
/// on a background thread.
pub fn spawn(addr: &str) -> io::Result<std::net::SocketAddr> {
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;
    thread::spawn(move || {
        let _ = serve(listener);
    });
    Ok(local)
}

fn handle(stream: TcpStream, hub: Arc<Mutex<Hub>>) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(HANDSHAKE))?;
    stream.set_nodelay(true)?;
    let mut socket = tungstenite::accept(stream)?;
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

    let result = pump(&mut socket, &hub, &document, id, &rx);

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
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match socket.read() {
            Ok(Frame::Text(text)) => {
                if let Ok(message) = serde_json::from_str::<Message>(text.as_str()) {
                    relay_message(hub, document, id, message)?;
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

/// Logs `message` and forwards it to the other clients of `document`. A repeated operation is
/// dropped without being forwarded again.
fn relay_message(
    hub: &Arc<Mutex<Hub>>,
    document: &str,
    from: u64,
    message: Message,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut hub = hub.lock().map_err(|_| "relay state poisoned")?;
    let outcome = hub.relay.publish(document, message.clone());
    if outcome == Publish::Duplicate {
        return Ok(());
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
    Ok(())
}
