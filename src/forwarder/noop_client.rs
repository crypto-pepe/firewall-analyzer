use crate::forwarder::ForwarderError;
use crate::model::BanRequest;
use crate::ExecutorClient;
use async_trait::async_trait;

pub struct NoopClient {}

#[async_trait]
impl ExecutorClient for NoopClient {
    async fn ban(&self, _br: BanRequest, _analyzer_id: String) -> Result<(), ForwarderError> {
        tracing::warn!("dry run mode");
        Ok(())
    }
}
