use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum CentralEvent {
    DeviceDiscovered {
        request: CentralRequest,
    },
    DeviceUpdated {
        request: CentralRequest,
    },
    DeviceConnected {
        request: CentralRequest,
    },
    DeviceDisconnected {
        request: CentralRequest,
    },
    ManufacturerDataAdvertisement {
        request: CentralRequest,
        manufacturer_data: HashMap<u16, Vec<u8>>,
    },
    ServiceDataAdvertisement {
        request: CentralRequest,
        service_data: HashMap<Uuid, Vec<u8>>,
    },
    ServicesAdvertisement {
        request: CentralRequest,
        services: Vec<Uuid>,
    },
    StateUpdate {
        request: CentralRequest,
        state: CentralState,
    },
}

#[derive(Debug, Clone)]
pub struct CentralRequest {
    pub server: String,
    pub service: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CentralState {
    Unknown = 0,
    PoweredOn = 1,
    PoweredOff = 2,
}
