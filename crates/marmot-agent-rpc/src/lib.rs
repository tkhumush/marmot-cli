#![doc = "JSON-RPC and gRPC server interfaces for marmot-agent"]

pub mod server;

pub struct RpcServer;

impl RpcServer {
    pub fn new() -> Self {
        Self
    }
}
