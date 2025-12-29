use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::{
    Result,
    api::{peripheral::PeripheralManager, peripheral_event::PeripheralEvent, service::Service},
    corebluetooth::objc_bindings::peripheral_manager_cb::ManagerEvent,
};

pub struct Peripheral {
    manager_tx: Sender<ManagerEvent>,
}

#[async_trait]
impl PeripheralManager for Peripheral {
    type PeripheralManager = Self;

    async fn new(sender_tx: Sender<PeripheralEvent>) -> Result<Self> {
        todo!()
    }

    async fn is_powered(&mut self) -> Result<bool> {
        todo!()
    }

    async fn is_advertising(&mut self) -> Result<bool> {
        todo!()
    }

    async fn start_advertising(&mut self, name: &str, uuids: &[Uuid]) -> Result<()> {
        todo!()
    }

    async fn stop_advertising(&mut self) -> Result<()> {
        todo!()
    }

    async fn add_service(&mut self, service: &Service) -> Result<()> {
        todo!()
    }

    async fn update_characteristic(&mut self, characteristic: Uuid, value: Vec<u8>) -> Result<()> {
        todo!()
    }
}
