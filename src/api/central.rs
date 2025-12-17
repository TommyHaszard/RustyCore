use crate::api::central_event::CentralEvent;
use crate::api::central_event::CentralState;
use crate::api::characteristic::Characteristic;
use crate::api::characteristic::CharacteristicProperty;
use crate::api::characteristic::CharacteristicWriteType;
use crate::api::descriptor::Descriptor;
use crate::api::service::Service;
use std::collections::BTreeSet;
use std::fmt::Debug;
use tokio::sync::mpsc::Sender;

use crate::Result;

use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait CentralManager: Send + Sync {
    type CentralManager: CentralManager;
    type Peripheral: PeripheralRemote;

    async fn new(sender_tx: Sender<CentralEvent>) -> Result<Self::CentralManager>;

    async fn start_scan(&mut self, filter: ScanFilter) -> Result<bool>;

    async fn stop_scan(&mut self) -> Result<()>;

    async fn peripherals(&mut self) -> Result<Vec<Self::Peripheral>>;

    async fn peripheral(&mut self, address: &PeripheralId) -> Result<Self::Peripheral>;

    async fn adapter_info(&mut self) -> Result<String>;

    async fn adapter_state(&mut self) -> Result<CentralState>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScanFilter {
    pub services: Vec<Uuid>,
}

#[async_trait]
pub trait PeripheralRemote: Send + Sync {
    type PeripheralRemote: PeripheralRemote;

    fn id(&self) -> PeripheralId;

    //fn address(&self) -> BDAddr;

    async fn properties(&self) -> Result<Option<CharacteristicProperty>>;

    fn services(&self) -> BTreeSet<Service>;

    fn characteristics(&self) -> BTreeSet<Characteristic> {
        self.services()
            .iter()
            .flat_map(|service| service.characteristics.clone().into_iter())
            .collect()
    }
    async fn is_connected(&self) -> Result<bool>;

    async fn connect(&self) -> Result<()>;

    async fn disconnect(&self) -> Result<()>;

    async fn discover_services(&self) -> Result<()>;

    async fn write(
        &self,
        characteristic: &Characteristic,
        data: &[u8],
        write_type: CharacteristicWriteType,
    ) -> Result<()>;

    async fn read(&self, characteristic: &Characteristic) -> Result<Vec<u8>>;

    // subscribe to notifications
    async fn subscribe(&self, characteristic: &Characteristic) -> Result<()>;

    // unsubscribe to notifications
    async fn unsubscribe(&self, characteristic: &Characteristic) -> Result<()>;

    async fn write_descriptor(&self, descriptor: &Descriptor, data: &[u8]) -> Result<()>;

    async fn read_descriptor(&self, descriptor: &Descriptor) -> Result<Vec<u8>>;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PeripheralId(Uuid);
