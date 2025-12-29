
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::Result;
use crate::api::peripheral_event::PeripheralEvent;
use crate::api::service::Service;

#[async_trait]
pub trait PeripheralManager: Send + Sync {
    type PeripheralManager: PeripheralManager;

    async fn new(sender_tx: Sender<PeripheralEvent>) -> Result<Self::PeripheralManager>;

    async fn is_powered(&mut self) -> Result<bool>;

    async fn is_advertising(&mut self) -> Result<bool>;

    async fn start_advertising(&mut self, name: &str, uuids: &[Uuid]) -> Result<()>;

    async fn stop_advertising(&mut self) -> Result<()>;

    async fn add_service(&mut self, service: &Service) -> Result<()>;

    async fn update_characteristic(&mut self, characteristic: Uuid, value: Vec<u8>) -> Result<()>;
}
