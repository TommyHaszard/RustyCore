use std::collections::BTreeSet;

use async_trait::async_trait;
use tokio::sync::{mpsc::Sender, oneshot};
use uuid::Uuid;

use crate::{
    Error, Result, api::{
        central::{CentralManager, PeripheralId, PeripheralRemote, ScanFilter},
        central_event::{CentralEvent, CentralState},
        characteristic::{Characteristic, CharacteristicProperty, CharacteristicWriteType},
        descriptor::Descriptor,
        service::Service,
    }
};

pub struct Central {
    peripherals: DashMap<PeripheralId, PeripheralRemote>,
    command_tx: Sender<CentralManagerCommand>,
}

#[async_trait]
impl CentralManager for Central {
    type CentralManager = Self;
    type Peripheral = Peripheral;

    async fn new(sender_tx: Sender<CentralEvent>) -> Result<Self> {
        todo!()
    }

    async fn start_scan(&mut self, filter: ScanFilter) -> Result<bool> {
        todo!()
    }

    async fn stop_scan(&mut self) -> Result<()> {
        todo!()
    }

    async fn peripherals(&mut self) -> Result<Vec<Self::Peripheral>> {
        todo!()
    }

    async fn peripheral(&mut self, address: &PeripheralId) -> Result<Self::Peripheral> {
        todo!()
    }

    async fn adapter_info(&mut self) -> Result<String> {
        todo!()
    }

    async fn adapter_state(&mut self) -> Result<CentralState> {
        todo!()
    }
}

pub struct Peripheral {
    command_tx: Sender<PeripheralRemoteCommand>,
}

#[async_trait]
impl PeripheralRemote for Peripheral {
    type PeripheralRemote = Self;

    fn id(&self) -> PeripheralId {
        todo!()
    }

    //fn address(&self) -> BDAddr {
    //todo!()
    //}

    async fn properties(&self) -> Result<Option<CharacteristicProperty>> {
        todo!()
    }

    fn services(&self) -> BTreeSet<Service> {
        todo!()
    }

    async fn is_connected(&self) -> Result<bool> {
        todo!()
    }

    async fn connect(&self) -> Result<()> {
        todo!()
    }

    async fn disconnect(&self) -> Result<()> {
        todo!()
    }

    async fn discover_services(&self) -> Result<()> {
        todo!()
    }

    async fn write(
        &self,
        characteristic: &Characteristic,
        data: &[u8],
        write_type: CharacteristicWriteType,
    ) -> Result<()> {
        todo!()
    }

    async fn read(&self, characteristic: &Characteristic) -> Result<Vec<u8>> {
        todo!()
    }

    // subscribe to notifications
    async fn subscribe(&self, characteristic: &Characteristic) -> Result<()> {
        todo!()
    }

    // unsubscribe to notifications
    async fn unsubscribe(&self, characteristic: &Characteristic) -> Result<()> {
        todo!()
    }

    async fn write_descriptor(&self, descriptor: &Descriptor, data: &[u8]) -> Result<()> {
        todo!()
    }

    async fn read_descriptor(&self, descriptor: &Descriptor) -> Result<Vec<u8>> {
        todo!()
    }
}

#[derive(Debug)]
pub enum PeripheralRemoteCommand {
     ConnectDevice {
        peripheral_uuid: Uuid,
        responder: oneshot::Sender<Result<bool>>,
    },
    DisconnectDevice {
        peripheral_uuid: Uuid,
        responder: oneshot::Sender<Result<bool>>,
    },
    ReadValue {
        peripheral_uuid: Uuid,
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        responder: oneshot::Sender<Result<bool>>,
    },
    WriteValue {
        peripheral_uuid: Uuid,
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        data: Vec<u8>,
        write_type: CharacteristicWriteType,
        responder: oneshot::Sender<Result<bool>>,
    },
    Subscribe {
        peripheral_uuid: Uuid,
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        responder: oneshot::Sender<Result<bool>>,
    },
    Unsubscribe {
        peripheral_uuid: Uuid,
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        responder: oneshot::Sender<Result<bool>>,
    },
    IsConnected {
        peripheral_uuid: Uuid,
        responder: oneshot::Sender<Result<bool>>,
    },
    ReadDescriptorValue {
        peripheral_uuid: Uuid,
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        descriptor_uuid: Uuid,
        responder: oneshot::Sender<Result<bool>>,
    },
    WriteDescriptorValue {
        peripheral_uuid: Uuid,
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        descriptor_uuid: Uuid,
        data: Vec<u8>,
        responder: oneshot::Sender<Result<bool>>,
    },
}

#[derive(Debug)]
pub enum CentralManagerCommand {
    GetAdapterState {
        responder: oneshot::Sender<Result<bool>>,
    },
    StartScanning {
        filter: ScanFilter,
    },
    StopScanning,
   }
