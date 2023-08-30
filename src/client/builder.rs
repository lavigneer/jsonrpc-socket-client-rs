use crate::client::transport::connect;
use std::time::Duration;

use jsonrpsee_core::client::IdKind;
use jsonrpsee_core::client::{Client, ClientBuilder};
use tokio::net::ToSocketAddrs;

pub struct SocketClientBuilder {
    id_kind: IdKind,
    max_concurrent_requests: usize,
    max_buffer_capacity_per_subscription: usize,
    max_log_length: u32,
    request_timeout: Duration,
}

impl Default for SocketClientBuilder {
    fn default() -> Self {
        Self {
            id_kind: IdKind::Number,
            max_log_length: 4096,
            max_concurrent_requests: 256,
            max_buffer_capacity_per_subscription: 1024,
            request_timeout: Duration::from_secs(60),
        }
    }
}

impl SocketClientBuilder {
    /// Create a new WASM client builder.
    pub fn new() -> SocketClientBuilder {
        SocketClientBuilder::default()
    }

    /// See documentation [`ClientBuilder::request_timeout`] (default is 60 seconds).
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// See documentation [`ClientBuilder::max_concurrent_requests`] (default is 256).
    pub fn max_concurrent_requests(mut self, max: usize) -> Self {
        self.max_concurrent_requests = max;
        self
    }

    /// See documentation [`ClientBuilder::max_buffer_capacity_per_subscription`] (default is 1024).
    pub fn max_buffer_capacity_per_subscription(mut self, max: usize) -> Self {
        self.max_buffer_capacity_per_subscription = max;
        self
    }

    /// See documentation for [`ClientBuilder::id_format`] (default is Number).
    pub fn id_format(mut self, kind: IdKind) -> Self {
        self.id_kind = kind;
        self
    }

    /// Set maximum length for logging calls and responses.
    ///
    /// Logs bigger than this limit will be truncated.
    pub fn set_max_logging_length(mut self, max: u32) -> Self {
        self.max_log_length = max;
        self
    }

    /// Build the client with specified URL to connect to.
    pub async fn build(self, url: impl ToSocketAddrs) -> Result<Client, jsonrpsee_core::Error> {
        let Self {
            max_log_length,
            id_kind,
            request_timeout,
            max_concurrent_requests,
            max_buffer_capacity_per_subscription,
        } = self;
        let (sender, receiver) = connect(url)
            .await
            .map_err(|e| jsonrpsee_core::Error::Transport(e.into()))?;

        let builder = ClientBuilder::default()
            .set_max_logging_length(max_log_length)
            .request_timeout(request_timeout)
            .id_format(id_kind)
            .max_buffer_capacity_per_subscription(max_buffer_capacity_per_subscription)
            .max_concurrent_requests(max_concurrent_requests);

        Ok(builder.build_with_tokio(sender, receiver))
    }
}
