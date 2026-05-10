//! Synchronous JSON-RPC server over TCP for marmot-agent daemon.
//!
//! Uses std::net::TcpListener with one thread per client.
//! This is a dev/testing server — simple, no async, no tokio.
//!
//! On startup it binds to the configured address (e.g. 127.0.0.1:9222).
//! If the address is "127.0.0.1:0" or similar, the OS assigns a free port;
//! the assigned port is printed so clients know where to connect.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use tracing::{info, warn};

/// JSON-RPC request.
#[derive(Debug, Deserialize)]
struct Request {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    id: Option<Value>,
}

/// JSON-RPC response.
#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl Response {
    fn ok(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    fn err(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError { code, message, data: None }),
            id,
        }
    }
}

/// Handler closure type: receives method name and params, returns JSON Value or error string.
pub type HandlerFn = Arc<
    dyn Fn(String, Value) -> Result<Value, String> + Send + Sync,
>;

/// Start a blocking JSON-RPC server on a TCP socket.
///
/// This runs in the current thread and blocks until an accept error occurs.
/// Run it in a dedicated thread from your daemon entry point.
///
/// If bind_address is "127.0.0.1:0", the OS picks a free port and the actual
/// listening address is printed.
pub fn serve_tcp_blocking(
    bind_address: &str,
    handler: HandlerFn,
) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(bind_address)?;
    let actual_addr = listener.local_addr()?;
    info!(
        "JSON-RPC server listening on {} (requested: {})",
        actual_addr, bind_address
    );
    println!(
        "Daemon listening on {} (requested: {})",
        actual_addr, bind_address
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let h = Arc::clone(&handler);
                thread::spawn(move || handle_client(stream, h));
            }
            Err(e) => {
                warn!("accept error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

fn handle_client(stream: TcpStream, handler: HandlerFn) {
    let mut reader = BufReader::new(&stream);
    let mut writer = &stream;
    let mut line = String::new();

    while let Ok(n) = reader.read_line(&mut line) {
        if n == 0 {
            break; // EOF
        }

        if line.trim().is_empty() {
            line.clear();
            continue;
        }

        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::err(None, -32700, format!("Parse error: {}", e));
                let _ = send_response(&mut writer, &resp);
                line.clear();
                continue;
            }
        };

        let id = req.id.clone();

        match handler(req.method, req.params) {
            Ok(result) => {
                let resp = Response::ok(id, result);
                let _ = send_response(&mut writer, &resp);
            }
            Err(msg) => {
                let resp = Response::err(id, -32000, msg);
                let _ = send_response(&mut writer, &resp);
            }
        }

        line.clear();
    }
}

fn send_response(
    writer: &mut dyn Write,
    resp: &Response,
) -> std::io::Result<()> {
    let mut json = match serde_json::to_string(resp) {
        Ok(s) => s,
        Err(e) => {
            let fallback = Response::err(resp.id.clone(), -32603, format!("Internal error: {}", e));
            serde_json::to_string(&fallback).unwrap_or_else(|_| "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32603,\"message\":\"Internal error\"}}".to_string())
        }
    };
    json.push('\n');
    writer.write_all(json.as_bytes())?;
    writer.flush()?;
    Ok(())
}
