use crate::api::central_event::{CentralEvent, CentralRequest, CentralState};
use crate::corebluetooth::objc_bindings::mac_extensions_cb::nsuuid_to_uuid;

use futures::executor;
use log::{error, trace};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, define_class, msg_send};
use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability, rc::Retained};
use objc2_core_bluetooth::{
    CBAdvertisementDataLocalNameKey, CBAdvertisementDataManufacturerDataKey,
    CBAdvertisementDataServiceDataKey, CBAdvertisementDataServiceUUIDsKey, CBCentralManager,
    CBCentralManagerDelegate, CBCharacteristic, CBDescriptor, CBManagerState, CBPeripheral,
    CBPeripheralDelegate, CBService, CBUUID,
};
use objc2_foundation::{
    NSArray, NSData, NSDictionary, NSError, NSNumber, NSObject, NSObjectProtocol, NSString,
};
use std::convert::TryInto;
use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    ops::Deref,
};
use tokio::sync::mpsc::Sender;

// Instance Variables that are stored within the ObjC class allowing communication between Rust
// code and the ObjC class.
#[derive(Debug)]
pub struct IVars {
    pub sender: Sender<CentralEvent>,
}

define_class!(
    #[derive(Debug)]
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    #[name = "CentralManagerDelegate"]
    #[ivars = IVars]
    pub struct CentralDelegate;

    unsafe impl NSObjectProtocol for CentralDelegate {}

    unsafe impl CBCentralManagerDelegate for CentralDelegate {
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
            let peripheral_uuid = nsuuid_to_uuid(retained_uuid);
            self.send_event(CentralEvent::DeviceConnected { server: peripheral_uuid });
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
            let peripheral_uuid = nsuuid_to_uuid(unsafe { &peripheral.identifier() });
            self.send_event(CentralEvent::DisconnectedDevice { peripheral_uuid });
        }

        #[unsafe(method(centralManager:didFailToConnectPeripheral:error:))]
        fn delegate_centralmanager_didfailtoconnectperipheral_error(
            &self,
            _central: &CBCentralManager,
            peripheral: &CBPeripheral,
            error: Option<&NSError>,
        ) {
            trace!("delegate_centralmanager_didfailtoconnectperipheral_error");
            let peripheral_uuid = nsuuid_to_uuid(unsafe { &peripheral.identifier() });
            let error_description = error.map(|error| error.localizedDescription().to_string());
            self.send_event(CentralEvent::ConnectionFailed {
                peripheral_uuid,
                error_description,
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

            let local_name = adv_data
                .get(unsafe { CBAdvertisementDataLocalNameKey })
                .map(|name| (name as *const AnyObject as *const NSString))
                .and_then(|name| unsafe { nsstring_to_string(name) });

            self.send_event(CentralEvent::DiscoveredPeripheral {
                cbperipheral: peripheral.retain(),
                local_name,
            });

            let rssi_value = rssi.as_i16();

            let peripheral_uuid = nsuuid_to_uuid(unsafe { &peripheral.identifier() });

            let manufacturer_data = adv_data.get(unsafe { CBAdvertisementDataManufacturerDataKey });
            if let Some(manufacturer_data) = manufacturer_data {
                // SAFETY: manufacturer_data is `NSData`
                let manufacturer_data: *const AnyObject = manufacturer_data;
                let manufacturer_data: *const NSData = manufacturer_data.cast();
                let manufacturer_data = unsafe { &*manufacturer_data };

                if manufacturer_data.len() >= 2 {
                    let (manufacturer_id, manufacturer_data) =
                        manufacturer_data.bytes().split_at(2);

                    self.send_event(CentralEvent::ManufacturerData {
                        peripheral_uuid,
                        manufacturer_id: u16::from_le_bytes(manufacturer_id.try_into().unwrap()),
                        data: Vec::from(manufacturer_data),
                        rssi: rssi_value,
                    });
                }
            }

            let service_data = adv_data.get(unsafe { CBAdvertisementDataServiceDataKey });
            if let Some(service_data) = service_data {
                // SAFETY: service_data is `NSDictionary<CBUUID, NSData>`
                let service_data: *const AnyObject = service_data;
                let service_data: *const NSDictionary<CBUUID, NSData> = service_data.cast();
                let service_data = unsafe { &*service_data };

                let mut result = HashMap::new();
                for uuid in service_data.keys() {
                    let data = &service_data[uuid];
                    result.insert(cbuuid_to_uuid(uuid), data.bytes().to_vec());
                }

                self.send_event(CentralEvent::ServiceData {
                    peripheral_uuid,
                    service_data: result,
                    rssi: rssi_value,
                });
            }

            let services = adv_data.get(unsafe { CBAdvertisementDataServiceUUIDsKey });
            if let Some(services) = services {
                // SAFETY: services is `NSArray<CBUUID>`
                let services: *const AnyObject = services;
                let services: *const NSArray<CBUUID> = services.cast();
                let services = unsafe { &*services };

                let mut service_uuids = Vec::new();
                for uuid in services {
                    service_uuids.push(cbuuid_to_uuid(uuid));
                }

                self.send_event(CentralEvent::Services {
                    peripheral_uuid,
                    service_uuids,
                    rssi: rssi_value,
                });
            }
        }
    }

    unsafe impl CBPeripheralDelegate for CentralDelegate {
        #[unsafe(method(peripheral:didDiscoverServices:))]
        fn delegate_peripheral_diddiscoverservices(
            &self,
            peripheral: &CBPeripheral,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_diddiscoverservices {} {}",
                peripheral_debug(peripheral),
                localized_description(error)
            );
            if error.is_none() {
                let services = unsafe { peripheral.services() }.unwrap_or_default();
                let mut service_map = HashMap::new();
                for s in services {
                    // go ahead and ask for characteristics and other services
                    unsafe {
                        peripheral.discoverCharacteristics_forService(None, &s);
                        peripheral.discoverIncludedServices_forService(None, &s);
                    }

                    // Create the map entry we'll need to export.
                    let uuid = cbuuid_to_uuid(unsafe { &s.UUID() });
                    service_map.insert(uuid, s);
                }
                let peripheral_uuid = nsuuid_to_uuid(unsafe { &peripheral.identifier() });
                self.send_event(CentralEvent::DiscoveredServices {
                    peripheral_uuid,
                    services: service_map,
                });
            }
        }

        #[unsafe(method(peripheral:didDiscoverIncludedServicesForService:error:))]
        fn delegate_peripheral_diddiscoverincludedservicesforservice_error(
            &self,
            peripheral: &CBPeripheral,
            service: &CBService,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_diddiscoverincludedservicesforservice_error {} {} {}",
                peripheral_debug(peripheral),
                service_debug(service),
                localized_description(error)
            );
            if error.is_none() {
                let includes = unsafe { service.includedServices() }.unwrap_or_default();
                for s in includes {
                    unsafe { peripheral.discoverCharacteristics_forService(None, &s) };
                }
            }
        }

        #[unsafe(method(peripheral:didDiscoverCharacteristicsForService:error:))]
        fn delegate_peripheral_diddiscovercharacteristicsforservice_error(
            &self,
            peripheral: &CBPeripheral,
            service: &CBService,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_diddiscovercharacteristicsforservice_error {} {} {}",
                peripheral_debug(peripheral),
                service_debug(service),
                localized_description(error)
            );
            if error.is_none() {
                let mut characteristics = HashMap::new();
                let chars = unsafe { service.characteristics() }.unwrap_or_default();
                for c in chars {
                    unsafe { peripheral.discoverDescriptorsForCharacteristic(&c) };
                    // Create the map entry we'll need to export.
                    let uuid = cbuuid_to_uuid(unsafe { &c.UUID() });
                    characteristics.insert(uuid, c);
                }
                let peripheral_uuid = nsuuid_to_uuid(unsafe { &peripheral.identifier() });
                let service_uuid = cbuuid_to_uuid(unsafe { &service.UUID() });
                self.send_event(CentralEvent::DiscoveredCharacteristics {
                    peripheral_uuid,
                    service_uuid,
                    characteristics,
                });
            }
        }

        #[unsafe(method(peripheral:didDiscoverDescriptorsForCharacteristic:error:))]
        fn delegate_peripheral_diddiscoverdescriptorsforcharacteristic_error(
            &self,
            peripheral: &CBPeripheral,
            characteristic: &CBCharacteristic,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_diddiscoverdescriptorsforcharacteristic_error {} {} {}",
                peripheral_debug(peripheral),
                characteristic_debug(characteristic),
                localized_description(error)
            );
            if error.is_none() {
                let mut descriptors = HashMap::new();
                let descs = unsafe { characteristic.descriptors() }.unwrap_or_default();
                for d in descs {
                    // Create the map entry we'll need to export.
                    let uuid = cbuuid_to_uuid(unsafe { &d.UUID() });
                    descriptors.insert(uuid, d);
                }
                let peripheral_uuid = nsuuid_to_uuid(unsafe { &peripheral.identifier() });
                let service = unsafe { characteristic.service() }.unwrap();
                let service_uuid = cbuuid_to_uuid(unsafe { &service.UUID() });
                let characteristic_uuid = cbuuid_to_uuid(unsafe { &characteristic.UUID() });
                self.send_event(CentralEvent::DiscoveredCharacteristicDescriptors {
                    peripheral_uuid,
                    service_uuid,
                    characteristic_uuid,
                    descriptors,
                });
            }
        }

        #[unsafe(method(peripheral:didUpdateValueForCharacteristic:error:))]
        fn delegate_peripheral_didupdatevalueforcharacteristic_error(
            &self,
            peripheral: &CBPeripheral,
            characteristic: &CBCharacteristic,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_didupdatevalueforcharacteristic_error {} {} {}",
                peripheral_debug(peripheral),
                characteristic_debug(characteristic),
                localized_description(error)
            );
            if error.is_none() {
                let service = unsafe { characteristic.service() }.unwrap();
                self.send_event(CentralEvent::CharacteristicNotified {
                    peripheral_uuid: nsuuid_to_uuid(unsafe { &peripheral.identifier() }),
                    service_uuid: cbuuid_to_uuid(unsafe { &service.UUID() }),
                    characteristic_uuid: cbuuid_to_uuid(unsafe { &characteristic.UUID() }),
                    data: get_characteristic_value(characteristic),
                });
                // Notify BluetoothGATTCharacteristic::read_value that read was successful.
            }
        }

        #[unsafe(method(peripheral:didWriteValueForCharacteristic:error:))]
        fn delegate_peripheral_didwritevalueforcharacteristic_error(
            &self,
            peripheral: &CBPeripheral,
            characteristic: &CBCharacteristic,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_didwritevalueforcharacteristic_error {} {} {}",
                peripheral_debug(peripheral),
                characteristic_debug(characteristic),
                localized_description(error)
            );
            if error.is_none() {
                let service = unsafe { characteristic.service() }.unwrap();
                self.send_event(CentralEvent::CharacteristicWritten {
                    peripheral_uuid: nsuuid_to_uuid(unsafe { &peripheral.identifier() }),
                    service_uuid: cbuuid_to_uuid(unsafe { &service.UUID() }),
                    characteristic_uuid: cbuuid_to_uuid(unsafe { &characteristic.UUID() }),
                });
            }
        }

        #[unsafe(method(peripheral:didUpdateNotificationStateForCharacteristic:error:))]
        fn delegate_peripheral_didupdatenotificationstateforcharacteristic_error(
            &self,
            peripheral: &CBPeripheral,
            characteristic: &CBCharacteristic,
            _error: Option<&NSError>,
        ) {
            trace!("delegate_peripheral_didupdatenotificationstateforcharacteristic_error");
            // TODO check for error here
            let peripheral_uuid = nsuuid_to_uuid(unsafe { &peripheral.identifier() });
            let service = unsafe { characteristic.service() }.unwrap();
            let service_uuid = cbuuid_to_uuid(unsafe { &service.UUID() });
            let characteristic_uuid = cbuuid_to_uuid(unsafe { &characteristic.UUID() });
            if unsafe { characteristic.isNotifying() } {
                self.send_event(CentralEvent::CharacteristicSubscribed {
                    peripheral_uuid,
                    service_uuid,
                    characteristic_uuid,
                });
            } else {
                self.send_event(CentralEvent::CharacteristicUnsubscribed {
                    peripheral_uuid,
                    service_uuid,
                    characteristic_uuid,
                });
            }
        }

        #[unsafe(method(peripheral:didReadRSSI:error:))]
        fn delegate_peripheral_didreadrssi_error(
            &self,
            peripheral: &CBPeripheral,
            _rssi: &NSNumber,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_didreadrssi_error {}",
                peripheral_debug(peripheral)
            );
            if error.is_none() {}
        }

        #[unsafe(method(peripheral:didUpdateValueForDescriptor:error:))]
        fn delegate_peripheral_didupdatevaluefordescriptor_error(
            &self,
            peripheral: &CBPeripheral,
            descriptor: &CBDescriptor,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_didupdatevaluefordescriptor_error {} {} {}",
                peripheral_debug(peripheral),
                descriptor_debug(descriptor),
                localized_description(error)
            );
            if error.is_none() {
                let characteristic = unsafe { descriptor.characteristic() }.unwrap();
                let service = unsafe { characteristic.service() }.unwrap();
                self.send_event(CentralEvent::DescriptorNotified {
                    peripheral_uuid: nsuuid_to_uuid(unsafe { &peripheral.identifier() }),
                    service_uuid: cbuuid_to_uuid(unsafe { &service.UUID() }),
                    characteristic_uuid: cbuuid_to_uuid(unsafe { &characteristic.UUID() }),
                    descriptor_uuid: cbuuid_to_uuid(unsafe { &descriptor.UUID() }),
                    data: get_descriptor_value(&descriptor),
                });
                // Notify BluetoothGATTCharacteristic::read_value that read was successful.
            }
        }

        #[unsafe(method(peripheral:didWriteValueForDescriptor:error:))]
        fn delegate_peripheral_didwritevaluefordescriptor_error(
            &self,
            peripheral: &CBPeripheral,
            descriptor: &CBDescriptor,
            error: Option<&NSError>,
        ) {
            trace!(
                "delegate_peripheral_didwritevaluefordescriptor_error {} {} {}",
                peripheral_debug(peripheral),
                descriptor_debug(descriptor),
                localized_description(error)
            );
            if error.is_none() {
                let characteristic = unsafe { descriptor.characteristic() }.unwrap();
                let service = unsafe { characteristic.service() }.unwrap();
                self.send_event(CentralEvent::DescriptorWritten {
                    peripheral_uuid: nsuuid_to_uuid(unsafe { &peripheral.identifier() }),
                    service_uuid: cbuuid_to_uuid(unsafe { &service.UUID() }),
                    characteristic_uuid: cbuuid_to_uuid(unsafe { &characteristic.UUID() }),
                    descriptor_uuid: cbuuid_to_uuid(unsafe { &descriptor.UUID() }),
                });
            }
        }
    }
);

impl CentralDelegate {
    pub fn new(sender: Sender<CentralEvent>) -> Retained<Self> {
        let this = CentralDelegate::alloc().set_ivars(IVars { sender });
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
