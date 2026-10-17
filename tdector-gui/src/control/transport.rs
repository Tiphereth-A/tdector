//! Authenticated loopback transport with bounded frames, requests and deadlines.
use super::protocol::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
};
use std::time::{Duration, Instant};

/// A session locator grants full control of its host. It must not enter diagnostics.
#[derive(Clone)]
pub struct SessionLocator {
    port: u16,
    token: String,
}
impl SessionLocator {
    pub fn parse(value: &str) -> Result<Self> {
        let mut parts = value.split(':');
        let prefix = parts.next();
        let port = parts
            .next()
            .and_then(|p| p.parse::<u16>().ok())
            .filter(|p| *p != 0);
        let token = parts
            .next()
            .filter(|t| t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()));
        if prefix != Some("tdector-gui-v1")
            || port.is_none()
            || token.is_none()
            || parts.next().is_some()
        {
            return Err(ControlError::session(
                "invalid_session",
                "Invalid GUI session locator",
            ));
        }
        Ok(Self {
            port: port.unwrap_or_default(),
            token: token.unwrap_or_default().to_owned(),
        })
    }
    pub fn encode(&self) -> String {
        format!("tdector-gui-v1:{}:{}", self.port, self.token)
    }
}

pub fn read_message<T: DeserializeOwned>(reader: impl Read) -> Result<T> {
    let mut reader = BufReader::new(reader).take((MAX_MESSAGE_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    reader
        .read_until(b'\n', &mut bytes)
        .map_err(|e| session_io("connection_failed", e))?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(ControlError::session(
            "message_too_large",
            "GUI control message exceeds 8 MiB",
        ));
    }
    if bytes.last() != Some(&b'\n') {
        return Err(ControlError::session(
            "protocol_error",
            "Incomplete GUI control message",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|e| ControlError::session("protocol_error", format!("Invalid control JSON: {e}")))
}
pub fn write_message(writer: &mut impl Write, message: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec(message)
        .map_err(|e| ControlError::session("protocol_error", e.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(ControlError::session(
            "message_too_large",
            "GUI control message exceeds 8 MiB",
        ));
    }
    writer
        .write_all(&bytes)
        .and_then(|_| writer.flush())
        .map_err(|e| session_io("connection_failed", e))
}
fn session_io(code: &str, error: std::io::Error) -> ControlError {
    ControlError::session(code, error.to_string())
}
fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| {
            ControlError::session("timeout", "Control request timed out before execution")
        })
}
/// Adjust the socket timeout before every read, so trickled input cannot extend a deadline.
struct DeadlineReader<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
}
impl Read for DeadlineReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let duration = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "control deadline elapsed")
            })?;
        self.stream.set_read_timeout(Some(duration))?;
        self.stream.read(bytes)
    }
}

/// Send exactly once. A missing response means outcome unknown, never a rollback.
pub fn send(locator: &str, command: String, args: Value, timeout: Duration) -> Result<Value> {
    let locator = SessionLocator::parse(locator)?;
    if timeout.is_zero() {
        return Err(ControlError::invalid("Timeout must be positive"));
    }
    let deadline = Instant::now() + timeout;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, locator.port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| session_io("connection_failed", e))?;
    stream
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(|e| session_io("connection_failed", e))?;
    let request = Request {
        protocol_version: PROTOCOL_VERSION,
        token: locator.token,
        command,
        args,
        timeout_ms: remaining(deadline)?
            .as_millis()
            .max(1)
            .min(u128::from(u64::MAX)) as u64,
    };
    // After the first write begins, even a write error can mean a request reached the host.
    write_message(&mut stream, &request).map_err(|error| {
        if matches!(error.code.as_str(), "message_too_large" | "protocol_error") {
            error
        } else {
            unknown_outcome(error)
        }
    })?;
    let response: Response = read_message(DeadlineReader {
        stream: &stream,
        deadline,
    })
    .map_err(unknown_outcome)?;
    if response.protocol_version != PROTOCOL_VERSION {
        return Err(ControlError::session(
            "protocol_mismatch",
            "GUI host uses a different protocol version",
        ));
    }
    response.result
}
fn unknown_outcome(error: ControlError) -> ControlError {
    let mut result = ControlError::session(
        "outcome_unknown",
        format!(
            "No confirmed GUI reply: {}. The command may have executed; inspect state before retrying.",
            error.message
        ),
    );
    result.details = Some(serde_json::json!({"cause": error.code, "retry_safe": false}));
    result
}

pub struct PendingRequest {
    pub command: String,
    pub args: Value,
    pub deadline: Instant,
    reply: SyncSender<Result<Value>>,
    pub written: Arc<AtomicBool>,
    state: Arc<AtomicUsize>,
}
impl PendingRequest {
    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
    /// Claim execution exactly once, racing cancellation at the queued deadline.
    pub fn begin(&self) -> bool {
        if self.expired() {
            let _ = self
                .state
                .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire);
            return false;
        }
        self.state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub fn respond(self, result: Result<Value>) {
        let _ = self.reply.send(result);
    }
}

/// Main-thread side of the host. Only owned JSON crosses this boundary.
pub struct HostBridge {
    receiver: Receiver<PendingRequest>,
    wake: Arc<Mutex<Option<egui::Context>>>,
    stop: Arc<AtomicBool>,
    pub session: String,
}
impl HostBridge {
    pub fn bind() -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|e| session_io("startup_failed", e))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| session_io("startup_failed", e))?;
        let port = listener
            .local_addr()
            .map_err(|e| session_io("startup_failed", e))?
            .port();
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| {
            ControlError::session(
                "startup_failed",
                format!("Cannot create session token: {e}"),
            )
        })?;
        let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let session = SessionLocator {
            port,
            token: token.clone(),
        }
        .encode();
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let wake = Arc::new(Mutex::new(None::<egui::Context>));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_wake, thread_stop) = (wake.clone(), stop.clone());
        std::thread::spawn(move || {
            let active = Arc::new(AtomicUsize::new(0));
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        if !peer.ip().is_loopback() {
                            continue;
                        }
                        if configure_accepted_stream(&stream).is_err() {
                            continue;
                        }
                        // One additional connection may report saturation, but never queue extra work.
                        if active.load(Ordering::Acquire) >= QUEUE_CAPACITY {
                            let mut stream = stream;
                            let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                            let _ = write_message(
                                &mut stream,
                                &Response::new(Err(ControlError::session(
                                    "queue_full",
                                    "GUI control request limit reached",
                                ))),
                            );
                            continue;
                        }
                        active.fetch_add(1, Ordering::AcqRel);
                        let (sender, token, wake, active) = (
                            sender.clone(),
                            token.clone(),
                            thread_wake.clone(),
                            active.clone(),
                        );
                        std::thread::spawn(move || {
                            handle_connection(stream, &token, &sender, &wake);
                            active.fetch_sub(1, Ordering::AcqRel);
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            receiver,
            wake,
            stop,
            session,
        })
    }
    pub fn set_context(&self, context: &egui::Context) {
        if let Ok(mut wake) = self.wake.lock() {
            *wake = Some(context.clone());
        }
    }
    pub fn try_recv(&self) -> Option<PendingRequest> {
        self.receiver.try_recv().ok()
    }
}
impl Drop for HostBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
fn configure_accepted_stream(stream: &TcpStream) -> std::io::Result<()> {
    // macOS inherits the listener's nonblocking mode. Connection workers use
    // blocking I/O with socket deadlines, including the saturation reply path.
    stream.set_nonblocking(false)
}
fn handle_connection(
    mut stream: TcpStream,
    token: &str,
    sender: &SyncSender<PendingRequest>,
    wake: &Arc<Mutex<Option<egui::Context>>>,
) {
    let admitted = Instant::now();
    let result = (|| {
        let request: Request = read_message(DeadlineReader {
            stream: &stream,
            deadline: admitted + Duration::from_millis(DEFAULT_TIMEOUT_MS),
        })?;
        if request.protocol_version != PROTOCOL_VERSION {
            return Err(ControlError::session(
                "protocol_mismatch",
                "Unsupported GUI control protocol version",
            ));
        }
        if !tokens_equal(request.token.as_bytes(), token.as_bytes()) {
            return Err(ControlError::session(
                "authentication_failed",
                "GUI session authentication failed",
            ));
        }
        if request.timeout_ms == 0 || request.timeout_ms > 86_400_000 {
            return Err(ControlError::invalid(
                "Timeout must be between 1 ms and 24 hours",
            ));
        }
        let deadline = admitted + Duration::from_millis(request.timeout_ms);
        remaining(deadline)?;
        let (reply, receiver) = mpsc::sync_channel(1);
        let written = Arc::new(AtomicBool::new(false));
        let state = Arc::new(AtomicUsize::new(0));
        let pending = PendingRequest {
            command: request.command,
            args: request.args,
            deadline,
            reply,
            written: written.clone(),
            state: state.clone(),
        };
        match sender.try_send(pending) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                return Err(ControlError::session(
                    "queue_full",
                    "GUI control queue is full",
                ));
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err(ControlError::session(
                    "session_closed",
                    "GUI session has closed",
                ));
            }
        }
        if let Ok(wake) = wake.lock()
            && let Some(ctx) = wake.as_ref()
        {
            ctx.request_repaint();
        }
        let result = receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_else(|_| {
            if state.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                Err(ControlError::session("timeout", "Command expired in the queue and was not executed"))
            } else {
                Err(ControlError::session("outcome_unknown", "No confirmed GUI response before deadline; execution may have begun. Inspect state before retrying."))
            }
        });
        let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
        let response = Response::new(result);
        if let Err(error) = write_message(&mut stream, &response)
            && error.code == "message_too_large"
        {
            let _ = write_message(&mut stream, &Response::new(Err(error)));
        }
        written.store(true, Ordering::Release);
        if let Ok(wake) = wake.lock()
            && let Some(ctx) = wake.as_ref()
        {
            ctx.request_repaint();
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
        let _ = write_message(&mut stream, &Response::new(Err(error)));
    }
}
fn tokens_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn bounded_frames_and_locators() {
        assert!(SessionLocator::parse("tdector-gui-v1:80:bad").is_err());
        assert!(read_message::<Value>(&b"{}"[..]).is_err());
        assert_eq!(
            read_message::<Value>("{\"word\":\"\u{732b}\"}\n".as_bytes()).expect("valid UTF-8"),
            json!({"word":"\u{732b}"})
        );
        let oversized = vec![b'x'; MAX_MESSAGE_BYTES + 1];
        assert_eq!(
            read_message::<Value>(&oversized[..])
                .expect_err("bounded")
                .code,
            "message_too_large"
        );
    }
    fn raw_request(host: &HostBridge, timeout_ms: u64) -> TcpStream {
        let locator = SessionLocator::parse(&host.session).expect("session locator");
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, locator.port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("read timeout");
        write_message(
            &mut stream,
            &Request {
                protocol_version: PROTOCOL_VERSION,
                token: locator.token,
                command: "set".into(),
                args: json!({}),
                timeout_ms,
            },
        )
        .expect("request");
        stream
    }
    fn next_request(host: &HostBridge) -> PendingRequest {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(request) = host.try_recv() {
                return request;
            }
            assert!(Instant::now() < deadline, "request never entered GUI queue");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn accepted_stream_waits_for_delayed_and_fragmented_requests() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("listener");
        let mut client =
            TcpStream::connect(listener.local_addr().expect("address")).expect("connect");
        let (stream, _) = listener.accept().expect("accept");
        // Reproduce inherited nonblocking mode on every platform.
        stream.set_nonblocking(true).expect("nonblocking stream");
        configure_accepted_stream(&stream).expect("blocking connection");
        client.set_nodelay(true).expect("disable buffering");
        client
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("read timeout");
        client
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("write timeout");
        let (sender, receiver) = mpsc::sync_channel(1);
        let (started, ready) = mpsc::sync_channel(0);
        let worker = std::thread::spawn(move || {
            started.send(()).expect("worker ready");
            handle_connection(stream, "test-token", &sender, &Arc::new(Mutex::new(None)));
        });
        ready
            .recv_timeout(Duration::from_secs(5))
            .expect("connection worker started");
        let mut bytes = serde_json::to_vec(&Request {
            protocol_version: PROTOCOL_VERSION,
            token: "test-token".into(),
            command: "set".into(),
            args: json!({"value": "delayed request"}),
            timeout_ms: 5000,
        })
        .expect("request JSON");
        bytes.push(b'\n');
        let split = bytes.len() / 2;
        for fragment in [&bytes[..split], &bytes[split..]] {
            // Before the first byte, and again before the rest of the frame,
            // the worker must wait instead of replying with a read error.
            let mut byte = [0];
            let error = client.read(&mut byte).expect_err("no premature reply");
            assert!(matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ));
            client.write_all(fragment).expect("request fragment");
        }
        let request = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("complete request entered GUI queue");
        assert_eq!(request.command, "set");
        assert_eq!(request.args, json!({"value": "delayed request"}));
        assert!(request.begin());
        request.respond(Ok(json!({"changed": true})));
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("reply timeout");
        let response: Response = read_message(client).expect("response");
        assert_eq!(
            response.result.expect("successful reply"),
            json!({"changed": true})
        );
        worker.join().expect("connection worker");
    }
    #[test]
    fn expired_queued_request_cannot_execute_later() {
        let host = HostBridge::bind().expect("host");
        let stream = raw_request(&host, 1000);
        let request = next_request(&host);
        let reply: Response = read_message(stream).expect("timeout reply");
        assert_eq!(reply.result.expect_err("expired").code, "timeout");
        assert!(
            !request.begin(),
            "cancelled queue entry must never mutate state"
        );
    }
    #[test]
    fn begun_request_timeout_reports_unknown_and_cannot_execute_twice() {
        let host = HostBridge::bind().expect("host");
        let stream = raw_request(&host, 1000);
        let request = next_request(&host);
        assert!(request.begin());
        assert!(!request.begin());
        let reply: Response = read_message(stream).expect("timeout reply");
        assert_eq!(
            reply
                .result
                .expect_err("begun without confirmed result")
                .code,
            "outcome_unknown"
        );
        // The main thread can finish once, but the client is never told it rolled back.
        request.respond(Ok(json!({"changed":true})));
    }
    #[test]
    fn host_rejects_saturation_and_keeps_queued_work_bounded() {
        let host = HostBridge::bind().expect("host");
        let mut clients = Vec::new();
        let mut requests = Vec::new();
        for _ in 0..QUEUE_CAPACITY {
            clients.push(raw_request(&host, 3000));
            requests.push(next_request(&host));
        }
        let locator = SessionLocator::parse(&host.session).expect("locator");
        let stream =
            TcpStream::connect((Ipv4Addr::LOCALHOST, locator.port)).expect("overflow connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("timeout");
        let response: Response = read_message(stream).expect("saturation reply");
        assert_eq!(response.result.expect_err("full").code, "queue_full");
        assert!(host.try_recv().is_none());
        for request in requests {
            assert!(request.begin());
            request.respond(Ok(json!({"accepted":true})));
        }
        for stream in clients {
            assert!(
                read_message::<Response>(stream)
                    .expect("reply")
                    .result
                    .is_ok()
            );
        }
    }
    #[test]
    fn sessions_are_isolated_and_protocol_versions_checked() {
        let first = HostBridge::bind().expect("first");
        let second = HostBridge::bind().expect("second");
        let first_locator = SessionLocator::parse(&first.session).expect("locator");
        let second_locator = SessionLocator::parse(&second.session).expect("locator");
        assert_ne!(first_locator.token, second_locator.token);
        let crossed = SessionLocator {
            port: first_locator.port,
            token: second_locator.token,
        };
        assert_eq!(
            send(
                &crossed.encode(),
                "status".into(),
                json!({}),
                Duration::from_secs(1)
            )
            .expect_err("cross-session token")
            .code,
            "authentication_failed"
        );
        let mut stream =
            TcpStream::connect((Ipv4Addr::LOCALHOST, first_locator.port)).expect("connect");
        write_message(
            &mut stream,
            &Request {
                protocol_version: PROTOCOL_VERSION + 1,
                token: first_locator.token,
                command: "status".into(),
                args: json!({}),
                timeout_ms: 1000,
            },
        )
        .expect("write");
        assert_eq!(
            read_message::<Response>(stream)
                .expect("response")
                .result
                .expect_err("version")
                .code,
            "protocol_mismatch"
        );
    }
    #[test]
    fn transport_authentication_and_single_execution() {
        let host = HostBridge::bind().expect("bind loopback");
        let locator = SessionLocator::parse(&host.session).expect("locator");
        let mut wrong = locator.clone();
        wrong.token = "0".repeat(64);
        assert_eq!(
            send(
                &wrong.encode(),
                "status".into(),
                json!({}),
                Duration::from_secs(1)
            )
            .expect_err("authentication")
            .code,
            "authentication_failed"
        );
        let session = host.session.clone();
        let client = std::thread::spawn(move || {
            send(
                &session,
                "set".into(),
                json!({"value":"a\nb 猫"}),
                Duration::from_secs(2),
            )
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        let pending = loop {
            if let Some(request) = host.try_recv() {
                break request;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(pending.args["value"], "a\nb 猫");
        pending.respond(Ok(json!({"changed": true})));
        assert_eq!(
            client.join().expect("client").expect("reply")["changed"],
            true
        );
        assert!(host.try_recv().is_none());
    }
}
