use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum CentralEvent {
    DeviceDiscovered {
        server: Uuid,
        name: String,
        rssi: i16,
    },
    DeviceUpdated {
        server: Uuid,
    },
    DeviceConnected {
        server: Uuid,
    },
    DeviceDisconnected {
        server: Uuid,
    },
    DeviceConnectionFailed {
        server: Uuid,
        error: Option<String>,
    },
    ManufacturerDataAdvertisement {
        server: Uuid,
        manufacturer_id: u16,
        manufacturer_data: Vec<u8>,
    },
    ServiceDataAdvertisement {
        server: Uuid,
        service_data: HashMap<Uuid, Vec<u8>>,
    },
    ServicesAdvertisement {
        server: Uuid,
        services: Vec<Uuid>,
    },
    StateUpdate {
        state: CentralState,
    },

}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CentralState {
    Unknown = 0,
    Resetting = 1,
    Unsupported = 2,
    Unauthorized = 3,
    PoweredOff = 4,
    PoweredOn = 5,
}
