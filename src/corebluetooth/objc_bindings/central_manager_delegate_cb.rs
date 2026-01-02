use crate::{
    api::central_event::{CentralEvent, CentralState},
    corebluetooth::objc_bindings::{AdvertisementResolver, ServiceResolver, mac_extensions_cb::{
        self,
    }},
};

use futures::executor;
use log::trace;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, define_class, msg_send};
use objc2::{DeclaredClass, rc::Retained};
use objc2_core_bluetooth::{
    CBAdvertisementDataLocalNameKey, CBAdvertisementDataManufacturerDataKey,
    CBAdvertisementDataServiceDataKey, CBAdvertisementDataServiceUUIDsKey, CBCentralManager,
    CBCentralManagerDelegate, CBCharacteristic, CBDescriptor, CBManagerState, CBPeripheral,
    CBPeripheralDelegate, CBService, CBUUID,
};
use objc2_foundation::{
    NSArray, NSData, NSDictionary, NSError, NSNumber, NSObject, NSObjectProtocol, NSString,
};
use uuid::Uuid;
use std::{convert::TryInto, sync::Arc};
use std::{collections::HashMap, fmt::Debug};
use tokio::sync::{Mutex, mpsc::Sender};

// Instance Variables that are stored within the ObjC class allowing communication between Rust
// code and the ObjC class.
#[derive(Debug)]
pub struct IVars {
    pub sender: Sender<CentralEvent>,
    pub services_resolver: Arc<Mutex<ServiceResolver>>,
    pub advertisment_resolver: Arc<Mutex<AdvertisementResolver>>,
}

define_class!(
    #[derive(Debug)]
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    #[name = "CentralManagerDelegate"]
    #[ivars = IVars]
    pub struct CentralManagerDelegate;

    unsafe impl NSObjectProtocol for CentralManagerDelegate {}

    unsafe impl CBCentralManagerDelegate for CentralManagerDelegate {
        #[unsafe(method(centralManagerDidUpdateState:))]
        fn delegate_centralmanagerdidupdatestate(&self, central: &CBCentralManager) {
            trace!("delegate_centralmanagerdidupdatestate");
            let state = unsafe { central.state() };
            let central_state = convert_state(state);
            self.send_event(CentralEvent::StateUpdate {
                state: central_state,
            });
        }

        // #[unsafe(method(centralManager:willRestoreState:))]
        // fn delegate_centralmanager_willrestorestate(&self, _central: &CBCentralManager, _dict: &NSDictionary<NSString, AnyObject>) {
        //     trace!("delegate_centralmanager_willrestorestate");
        // }

        #[unsafe(method(centralManager:didConnectPeripheral:))]
        fn delegate_centralmanager_didconnectperipheral(
            &self,
            _central: &CBCentralManager,
            peripheral: &CBPeripheral,
        ) {
            trace!(
                "delegate_centralmanager_didconnectperipheral {}",
                peripheral_debug(peripheral)
            );
            unsafe { peripheral.setDelegate(Some(ProtocolObject::from_ref(self))) };
            unsafe { peripheral.discoverServices(None) }
            let retained_uuid = unsafe { &peripheral.identifier() };
            let peripheral_uuid = mac_extensions_cb::nsuuid_to_uuid(retained_uuid);
            self.send_event(CentralEvent::DeviceConnected {
                server: peripheral_uuid,
            });
        }

        #[unsafe(method(centralManager:didDisconnectPeripheral:error:))]
        fn delegate_centralmanager_diddisconnectperipheral_error(
            &self,
            _central: &CBCentralManager,
            peripheral: &CBPeripheral,
            _error: Option<&NSError>,
        ) {
            trace!(
                "delegate_centralmanager_diddisconnectperipheral_error {}",
                peripheral_debug(peripheral)
            );
            let retained_uuid = unsafe { &peripheral.identifier() };
            let peripheral_uuid = mac_extensions_cb::nsuuid_to_uuid(retained_uuid);
            self.send_event(CentralEvent::DeviceDisconnected {
                server: peripheral_uuid,
            });
        }

        #[unsafe(method(centralManager:didFailToConnectPeripheral:error:))]
        fn delegate_centralmanager_didfailtoconnectperipheral_error(
            &self,
            _central: &CBCentralManager,
            peripheral: &CBPeripheral,
            error: Option<&NSError>,
        ) {
            trace!("delegate_centralmanager_didfailtoconnectperipheral_error");
            let retained_uuid = unsafe { &peripheral.identifier() };
            let peripheral_uuid = mac_extensions_cb::nsuuid_to_uuid(retained_uuid);
            let error_description = error.map(|error| error.localizedDescription().to_string());
            self.send_event(CentralEvent::DeviceConnectionFailed {
                server: peripheral_uuid,
                error: error_description,
            });
        }

        #[unsafe(method(centralManager:didDiscoverPeripheral:advertisementData:RSSI:))]
        fn delegate_centralmanager_diddiscoverperipheral_advertisementdata_rssi(
            &self,
            _central: &CBCentralManager,
            peripheral: &CBPeripheral,
            adv_data: &NSDictionary<NSString, AnyObject>,
            rssi: &NSNumber,
        ) {
            trace!(
                "delegate_centralmanager_diddiscoverperipheral_advertisementdata_rssi {}",
                peripheral_debug(peripheral)
            );

            let local_name = unsafe {
                adv_data
                    .objectForKey(CBAdvertisementDataLocalNameKey)
                    .and_then(|name_obj| {
                        let name_nsstring: *const NSString =
                            Retained::<AnyObject>::as_ptr(&name_obj) as *const NSString;
                        mac_extensions_cb::nsstring_to_string(name_nsstring)
                    })
            }
            .unwrap_or_else(|| String::from("Unknown"));

            let retained_uuid = unsafe { &peripheral.identifier() };
            let peripheral_uuid = mac_extensions_cb::nsuuid_to_uuid(retained_uuid);

            let rssi_value = rssi.as_i16();

            self.send_event(CentralEvent::DeviceDiscovered {
                server: peripheral_uuid,
                name: local_name,
                rssi: rssi_value,
            });

            let manufacturer_data =
                unsafe { adv_data.objectForKey(CBAdvertisementDataManufacturerDataKey) };

            if let Some(manufacturer_data) = manufacturer_data {
                // SAFETY: manufacturer_data is `NSData`
                let manufacturer_data_ptr: *const AnyObject = Retained::as_ptr(&manufacturer_data);
                let manufacturer_data_nsdata: *const NSData = manufacturer_data_ptr.cast();
                let manufacturer_data: &NSData = unsafe { &*manufacturer_data_nsdata };

                if manufacturer_data.len() >= 2 {
                    let (manufacturer_id, manufacturer_data) =
                        unsafe { manufacturer_data.as_bytes_unchecked().split_at(2) };

                    self.send_event(CentralEvent::ManufacturerDataAdvertisement {
                        server: peripheral_uuid,
                        manufacturer_id: u16::from_le_bytes(manufacturer_id.try_into().unwrap()),
                        manufacturer_data: Vec::from(manufacturer_data),
                    });
                }
            }

            let service_data = unsafe { adv_data.objectForKey(CBAdvertisementDataServiceDataKey) };

            if let Some(service_data) = service_data {
                // SAFETY: service_data is `NSDictionary<CBUUID, NSData>`
                let service_data: *const AnyObject = Retained::as_ptr(&service_data);
                let service_data: *const NSDictionary<CBUUID, NSData> = service_data.cast();
                let service_data: &NSDictionary<CBUUID, NSData> = unsafe { &*service_data };

                let mut result = HashMap::new();
                for cbuuid in service_data.keys() {
                    if let Some(data_obj) = service_data.objectForKey(&cbuuid) {
                        let data = unsafe {
                            let data_ptr = Retained::as_ptr(&data_obj) as *const NSData;
                            let nsdata = &*data_ptr;
                            nsdata.as_bytes_unchecked().to_vec()
                        };
                        let service_uuid: Uuid = unsafe {mac_extensions_cb::cbuuid_to_uuid(&cbuuid) };
                        result.insert(service_uuid, data);
                    }
                }

                self.send_event(CentralEvent::ServiceDataAdvertisement {
                    server: peripheral_uuid,
                    service_data: result,
                });
            }

            let services = unsafe {adv_data.objectForKey( CBAdvertisementDataServiceUUIDsKey)};
            if let Some(services) = services {
                // SAFETY: services is `NSArray<CBUUID>`
                let services: *const AnyObject = Retained::as_ptr(&services);
                let services: *const NSArray<CBUUID> = services.cast();
                let services: &NSArray<CBUUID> = unsafe { &*services };

                let mut service_uuids = Vec::new();
                for cbuuid in services {
                    let service_uuid: Uuid = unsafe {mac_extensions_cb::cbuuid_to_uuid(&cbuuid) };
                    service_uuids.push(service_uuid);
                }

                self.send_event(CentralEvent::ServicesAdvertisement {
                    server: peripheral_uuid,
                    services: service_uuids,
                });
            }
        }
    }
);

impl CentralManagerDelegate {
    pub fn new(sender: Sender<CentralEvent>) -> Retained<Self> {
        let this = CentralManagerDelegate::alloc().set_ivars(IVars { sender });
        unsafe { msg_send![super(this), init] }
    }

    fn send_event(&self, event: CentralEvent) {
        let sender = self.ivars().sender.clone();
        executor::block_on(async {
            if let Err(e) = sender.send(event).await {
                log::error!("Error sending delegate event: {}", e);
            }
        });
    }
}

fn localized_description(error: Option<&NSError>) -> String {
    if let Some(error) = error {
        error.localizedDescription().to_string()
    } else {
        "".to_string()
    }
}

fn peripheral_debug(peripheral: &CBPeripheral) -> String {
    let uuid = unsafe { peripheral.identifier() }.UUIDString();
    if let Some(name) = unsafe { peripheral.name() } {
        format!("CBPeripheral({}, {})", name, uuid)
    } else {
        format!("CBPeripheral({})", uuid)
    }
}

fn service_debug(service: &CBService) -> String {
    let uuid = unsafe { service.UUID().UUIDString() };
    format!("CBService({})", uuid)
}

fn characteristic_debug(characteristic: &CBCharacteristic) -> String {
    let uuid = unsafe { characteristic.UUID().UUIDString() };
    format!("CBCharacteristic({})", uuid)
}

fn descriptor_debug(descriptor: &CBDescriptor) -> String {
    let uuid = unsafe { descriptor.UUID().UUIDString() };
    format!("CBDescriptor({})", uuid)
}

fn convert_state(cb_state: CBManagerState) -> CentralState {
    match cb_state {
        CBManagerState::Unknown => CentralState::Unknown,
        CBManagerState::Resetting => CentralState::Resetting,
        CBManagerState::Unsupported => CentralState::Unsupported,
        CBManagerState::Unauthorized => CentralState::Unauthorized,
        CBManagerState::PoweredOff => CentralState::PoweredOff,
        CBManagerState::PoweredOn => CentralState::PoweredOn,
        _ => {
            log::warn!("Unexpected CBManagerState value, treating as Unknown");
            CentralState::Unknown
        }
    }
}
