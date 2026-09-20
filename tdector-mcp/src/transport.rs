//! Bounded stdio framing with SDK message decoding and protocol error responses.

use crate::worker::Shutdown;
use rmcp::{
    RoleServer,
    model::{ClientRequest, ErrorData, RequestId},
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::{Transport, async_rw::JsonRpcMessageCodec},
};
use serde_json::Value;
use std::sync::Arc;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf},
    sync::Mutex,
    task::JoinHandle,
};
use tokio_util::{
    bytes::BytesMut,
    codec::{Decoder, Encoder},
};

pub const OUTPUT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
pub const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);

pub struct BoundedInput<R> {
    inner: R,
    limit: usize,
    line_bytes: usize,
    shutdown: Shutdown,
}

impl<R> BoundedInput<R> {
    pub fn new(inner: R, limit: usize, shutdown: Shutdown) -> Self {
        Self {
            inner,
            limit,
            line_bytes: 0,
            shutdown,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for BoundedInput<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let mut bytes = [0u8; 8192];
        let length = output.remaining().min(bytes.len());
        let mut input = ReadBuf::new(&mut bytes[..length]);
        match Pin::new(&mut this.inner).poll_read(cx, &mut input) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => {
                this.shutdown.stop();
                Poll::Ready(Err(error))
            }
            Poll::Ready(Ok(())) => {
                if input.filled().is_empty() {
                    this.shutdown.stop();
                }
                for byte in input.filled() {
                    if *byte == b'\n' {
                        this.line_bytes = 0;
                    } else {
                        this.line_bytes += 1;
                        if this.line_bytes > this.limit {
                            this.shutdown.stop();
                            eprintln!(
                                "tdector-mcp: incoming protocol line exceeds {} bytes; closing transport",
                                this.limit
                            );
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "MCP message byte limit exceeded",
                            )));
                        }
                    }
                }
                output.put_slice(input.filled());
                Poll::Ready(Ok(()))
            }
        }
    }
}

/// Decode with the SDK while reporting malformed input instead of silently discarding it. Reads retain partial lines across cancellation by the service loop; all response writers share one lock and flush complete newline frames.
pub struct StdioTransport<R: AsyncRead, W> {
    read: BufReader<BoundedInput<R>>,
    line: Vec<u8>,
    write: Arc<Mutex<Option<W>>>,
    pending_error: Option<JoinHandle<io::Result<()>>>,
    shutdown: Shutdown,
    closed: bool,
}

impl<R: AsyncRead + Unpin, W> StdioTransport<R, W> {
    pub fn new(read: BoundedInput<R>, write: W) -> Self {
        let shutdown = read.shutdown.clone();
        Self {
            read: BufReader::new(read),
            line: Vec::new(),
            write: Arc::new(Mutex::new(Some(write))),
            pending_error: None,
            shutdown,
            closed: false,
        }
    }
}

async fn write_message<W: AsyncWrite + Unpin>(
    writer: Arc<Mutex<Option<W>>>,
    item: TxJsonRpcMessage<RoleServer>,
) -> io::Result<()> {
    let mut bytes = BytesMut::new();
    JsonRpcMessageCodec::default()
        .encode(item, &mut bytes)
        .map_err(io::Error::from)?;
    let mut locked = tokio::time::timeout(OUTPUT_TIMEOUT, writer.lock())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "MCP output lock timed out"))?;
    let result = {
        let writer = locked
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Transport is closed"))?;
        tokio::time::timeout(OUTPUT_TIMEOUT, async {
            writer.write_all(&bytes).await?;
            writer.flush().await
        })
        .await
    };
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            locked.take();
            Err(error)
        }
        Err(_) => {
            // A stalled or partially written stream cannot be reused. Releasing this writer also lets close finish after its caller observes EOF.
            locked.take();
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "MCP output write timed out",
            ))
        }
    }
}

/// Validate envelope fields before an untagged SDK enum could reinterpret an invalid request as a notification. Message-specific parsing stays in rmcp.
fn valid_envelope(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return false;
    }
    if object
        .get("id")
        .is_some_and(|id| serde_json::from_value::<RequestId>(id.clone()).is_err())
    {
        return false;
    }
    let kind_count = ["method", "result", "error"]
        .iter()
        .filter(|key| object.contains_key(**key))
        .count();
    if kind_count != 1 {
        return false;
    }
    if let Some(method) = object.get("method") {
        return method.is_string()
            && object
                .get("params")
                .is_none_or(|params| params.is_object() || params.is_array());
    }
    if object.contains_key("result") {
        return object.contains_key("id");
    }
    object.get("error").is_some_and(Value::is_object)
}

fn decode_line(
    line: &[u8],
) -> Result<Option<RxJsonRpcMessage<RoleServer>>, (ErrorData, Option<RequestId>)> {
    let trimmed = line.strip_suffix(b"\n").unwrap_or(line);
    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
    let trimmed = trimmed.strip_prefix(b"\xef\xbb\xbf").unwrap_or(trimmed);
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value: Value = serde_json::from_slice(trimmed)
        .map_err(|_| (ErrorData::parse_error("Parse error", None), None))?;
    let id = value
        .get("id")
        .and_then(|id| serde_json::from_value(id.clone()).ok());
    if !valid_envelope(&value) {
        return Err((ErrorData::invalid_request("Invalid request", None), id));
    }
    let mut bytes = BytesMut::from(line);
    let message = JsonRpcMessageCodec::<RxJsonRpcMessage<RoleServer>>::default()
        .decode_eof(&mut bytes)
        .map_err(|_| {
            (
                ErrorData::invalid_request("Invalid request", None),
                id.clone(),
            )
        })?;
    if let Some(RxJsonRpcMessage::<RoleServer>::Request(request)) = &message
        && let ClientRequest::CustomRequest(custom) = &request.request
        && matches!(
            custom.method.as_str(),
            "initialize" | "ping" | "tools/list" | "tools/call"
        )
    {
        return Err((
            ErrorData::invalid_request("Invalid request parameters", None),
            id,
        ));
    }
    Ok(message)
}

impl<R, W> Transport<RoleServer> for StdioTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send + 'static,
{
    type Error = io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = io::Result<()>> + Send + 'static {
        let writer = Arc::clone(&self.write);
        let shutdown = self.shutdown.clone();
        async move {
            let result = write_message(writer, item).await;
            if let Err(error) = &result {
                eprintln!("tdector-mcp: protocol output failed: {error}");
                shutdown.stop();
            }
            result
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleServer>> {
        loop {
            if let Some(pending) = self.pending_error.as_mut() {
                let result = pending.await;
                self.pending_error = None;
                if !matches!(result, Ok(Ok(()))) {
                    self.closed = true;
                    self.shutdown.stop();
                    return None;
                }
            }
            if self.closed {
                return None;
            }
            match self.read.read_until(b'\n', &mut self.line).await {
                Ok(0) => {
                    self.closed = true;
                    self.shutdown.stop();
                    return None;
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("tdector-mcp: protocol input failed: {error}");
                    self.line.clear();
                    self.closed = true;
                    self.shutdown.stop();
                    return None;
                }
            }
            let decoded = decode_line(&self.line);
            self.line.clear();
            match decoded {
                Ok(Some(message)) => return Some(message),
                Ok(None) => continue,
                Err((error, id)) => {
                    // Receive futures are cancelled whenever another service event wins select!. Keep this send alive and resume awaiting it so cancellation can neither lose the reply nor split a frame.
                    self.pending_error =
                        Some(tokio::spawn(
                            self.send(TxJsonRpcMessage::<RoleServer>::error(error, id)),
                        ));
                }
            }
        }
    }

    async fn close(&mut self) -> io::Result<()> {
        self.closed = true;
        if let Some(pending) = self.pending_error.take() {
            pending.await.map_err(io::Error::other)??;
        }
        let mut locked = self.write.lock().await;
        if let Some(mut writer) = locked.take() {
            writer.flush().await?;
            writer.shutdown().await?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn rejects_oversized_unterminated_lines_and_counts_each_line() {
        let mut input = BoundedInput::new(&b"1234\n1234\n"[..], 4, Shutdown::default());
        let mut text = String::new();
        input
            .read_to_string(&mut text)
            .await
            .expect("bounded lines");
        assert_eq!(text, "1234\n1234\n");
        let mut input = BoundedInput::new(&b"12345"[..], 4, Shutdown::default());
        assert_eq!(
            input
                .read_to_end(&mut Vec::new())
                .await
                .expect_err("over limit")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use rmcp::model::ErrorCode;
    use std::time::Duration;
    use tokio::io::AsyncReadExt;

    #[test]
    fn malformed_json_and_invalid_envelopes_have_distinct_protocol_errors() {
        for line in [b"not JSON\n".as_slice(), b"{\n", b"\xff\n"] {
            let (error, id) = decode_line(line).expect_err("parse error");
            assert_eq!(error.code, ErrorCode::PARSE_ERROR);
            assert!(id.is_none());
        }
        for value in [
            serde_json::json!([]),
            serde_json::json!({"jsonrpc":"1.0", "method":"ping", "id":12}),
            serde_json::json!({"jsonrpc":"2.0", "method":42, "id":12}),
            serde_json::json!({"jsonrpc":"2.0", "method":"ping", "id":false}),
            serde_json::json!({"jsonrpc":"2.0", "method":"ping", "id":null}),
            serde_json::json!({"jsonrpc":"2.0", "method":"ping", "result":{}, "id":12}),
            serde_json::json!({"jsonrpc":"2.0", "method":"tools/call", "params":{}, "id":12}),
            serde_json::json!({"jsonrpc":"2.0", "method":"tools/call", "params":false, "id":12}),
            serde_json::json!({"jsonrpc":"2.0", "method":"initialize", "params":{}, "id":12}),
            serde_json::json!({"jsonrpc":"2.0", "result":{}}),
        ] {
            let bytes = serde_json::to_vec(&value).expect("invalid fixture JSON");
            let (error, _) = decode_line(&bytes).expect_err("invalid request");
            assert_eq!(error.code, ErrorCode::INVALID_REQUEST, "{value}");
        }
    }

    #[test]
    fn sdk_decoding_retains_bom_crlf_and_unknown_method_compatibility() {
        let message =
            decode_line(b"\xef\xbb\xbf{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"id\":7}\r\n")
                .expect("BOM CRLF decoding")
                .expect("ping message");
        assert_eq!(
            serde_json::to_value(message).expect("message JSON")["id"],
            7
        );
        let message =
            decode_line(b"{\"jsonrpc\":\"2.0\",\"method\":\"example/unknown\",\"id\":8}\n")
                .expect("unknown method stays dispatchable")
                .expect("custom request");
        assert!(
            matches!(message, RxJsonRpcMessage::<RoleServer>::Request(request) if matches!(request.request, ClientRequest::CustomRequest(_)))
        );
    }

    #[tokio::test]
    async fn malformed_input_gets_a_reply_and_next_request_is_delivered() {
        let (server, client) = tokio::io::duplex(4096);
        let (server_read, server_write) = tokio::io::split(server);
        let (client_read, mut client_write) = tokio::io::split(client);
        let mut transport = StdioTransport::new(
            BoundedInput::new(server_read, 1024, Shutdown::default()),
            server_write,
        );
        client_write.write_all(b"not JSON\n{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{},\"id\":4}\n{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"id\":5}\n")
            .await.expect("write protocol fixtures");
        let message = transport.receive().await.expect("next valid request");
        assert_eq!(
            serde_json::to_value(message).expect("message JSON")["id"],
            5
        );
        let mut peer = BufReader::new(client_read);
        for (code, id) in [(-32700, None), (-32600, Some(4))] {
            let mut line = String::new();
            peer.read_line(&mut line)
                .await
                .expect("protocol error response");
            let value: Value = serde_json::from_str(&line).expect("error JSON");
            assert_eq!(value["error"]["code"], code);
            assert_eq!(value.get("id").and_then(Value::as_i64), id);
        }
    }

    #[tokio::test]
    async fn interrupted_receive_keeps_partial_input() {
        let (server_read, mut peer) = tokio::io::duplex(4096);
        let mut transport = StdioTransport::new(
            BoundedInput::new(server_read, 1024, Shutdown::default()),
            tokio::io::sink(),
        );
        peer.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":")
            .await
            .expect("partial request");
        assert!(
            tokio::time::timeout(Duration::from_millis(10), transport.receive())
                .await
                .is_err()
        );
        peer.write_all(b"\"ping\",\"id\":9}\n")
            .await
            .expect("finish request");
        let message = transport.receive().await.expect("partial bytes retained");
        assert_eq!(
            serde_json::to_value(message).expect("message JSON")["id"],
            9
        );
    }

    #[tokio::test]
    async fn interrupted_protocol_error_send_finishes_one_complete_frame() {
        let (mut peer_input, server_read) = tokio::io::duplex(4096);
        let (server_write, peer_output) = tokio::io::duplex(1);
        let mut transport = StdioTransport::new(
            BoundedInput::new(server_read, 1024, Shutdown::default()),
            server_write,
        );
        peer_input
            .write_all(b"bad JSON\n{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"id\":10}\n")
            .await
            .expect("malformed then valid request");
        // The error frame cannot fit until its reader runs. Dropping receive during that write must leave the pending send task and lock intact.
        assert!(
            tokio::time::timeout(Duration::from_millis(10), transport.receive())
                .await
                .is_err()
        );
        let mut peer = BufReader::new(peer_output);
        let mut error_line = String::new();
        let (message, read) = tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(transport.receive(), peer.read_line(&mut error_line))
        })
        .await
        .expect("cancelled receive resumes");
        read.expect("read complete protocol error");
        let error: Value = serde_json::from_str(&error_line).expect("uninterrupted error JSON");
        assert_eq!(error["error"]["code"], -32700);
        assert_eq!(
            serde_json::to_value(message.expect("next ping")).expect("message JSON")["id"],
            10
        );
    }

    #[tokio::test]
    async fn eof_closes_and_concurrent_sends_never_interleave() {
        let (server_write, mut peer) = tokio::io::duplex(4096);
        let mut transport = StdioTransport::new(
            BoundedInput::new(tokio::io::empty(), 1024, Shutdown::default()),
            server_write,
        );
        let first = transport.send(TxJsonRpcMessage::<RoleServer>::error(
            ErrorData::parse_error("first", None),
            None,
        ));
        let second = transport.send(TxJsonRpcMessage::<RoleServer>::error(
            ErrorData::invalid_request("second", None),
            None,
        ));
        let (first, second) = tokio::join!(first, second);
        first.expect("first frame");
        second.expect("second frame");
        assert!(transport.receive().await.is_none());
        transport.close().await.expect("close output");
        let mut output = String::new();
        peer.read_to_string(&mut output)
            .await
            .expect("read output to EOF");
        let frames: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).expect("complete frame"))
            .collect();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0]["error"]["message"], "first");
        assert_eq!(frames[1]["error"]["message"], "second");
    }
}
