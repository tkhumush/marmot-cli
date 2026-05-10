#![doc = "JSON-RPC and daemon interfaces for marmot-agent"]

pub mod server;

use serde_json::Value;

/// JSON-RPC method handler type alias.
pub type MethodHandler = Box<
    dyn Fn(String, Value) -> Box<
        dyn std::future::Future<Output = Result<Value, String>> + Send
    > + Send + Sync,
>;
