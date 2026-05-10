
pub struct JsonRpcConfig {
    pub socket_path: String,
}

impl Default for JsonRpcConfig {
    fn default() -> Self {
        Self {
            socket_path: "/tmp/marmot-agent.sock".to_string(),
        }
    }
}
