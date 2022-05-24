use crate::forwarder::ForwarderError;
use crate::model::BanRequest;
use crate::ExecutorClient;
use async_trait::async_trait;

pub struct NoopClient {}

#[async_trait]
impl ExecutorClient for NoopClient {
    async fn send_ban_request(&self, _br: BanRequest) -> Result<(), ForwarderError> {
        tracing::warn!("dry run mod");
        Ok(())
    }
}
