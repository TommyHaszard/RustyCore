use crate::corebluetooth::objc_bindings::{
    AdvertisementResolver, ServiceResolver,
    mac_extensions_cb::{self},
};

use futures::executor;
use log::trace;
use objc2::{AnyThread, define_class, msg_send};
use objc2::{DeclaredClass, rc::Retained};
use objc2_core_bluetooth::{
    CBCharacteristic, CBDescriptor, CBPeripheral, CBPeripheralDelegate, CBService,
};
use objc2_foundation::{NSError, NSNumber, NSObject, NSObjectProtocol};
use std::sync::Arc;
use std::{collections::HashMap, fmt::Debug};
use tokio::sync::{Mutex, mpsc::Sender};
use uuid::Uuid;

// Instance Variables that are stored within the ObjC class allowing communication between Rust
// code and the ObjC class.
#[derive(Debug)]
pub struct IVars {
    pub sender: Sender<PeripheralDelegateEvent>,
    pub services_resolver: Arc<Mutex<ServiceResolver>>,
    pub advertisement_resolver: Arc<Mutex<AdvertisementResolver>>,
}

define_class!(
    #[derive(Debug)]
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    #[name = "PeripheralDelegate"]
    #[ivars = IVars]
    pub struct PeripheralDelegate;

    unsafe impl NSObjectProtocol for PeripheralDelegate {}

    unsafe impl CBPeripheralDelegate for PeripheralDelegate {
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
            let services = unsafe { peripheral.services() }.unwrap_or_default();
            let mut service_map = HashMap::new();
            for s in services {
                // go ahead and ask for characteristics and other services
                unsafe {
                    let uuid = mac_extensions_cb::cbuuid_to_uuid(&s.UUID());
                    service_map.insert(uuid, s);
                }
            }
            let peripheral_uuid =
                mac_extensions_cb::nsuuid_to_uuid(unsafe { &peripheral.identifier() });
            self.send_event(PeripheralDelegateEvent::DiscoveredServices {
                services: service_map,
                error: error.map(|e| e.localizedDescription().to_string()),
            });
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
            // TODO: make ths a oneshot and pull this logic into the peripheral
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
            let mut characteristics = HashMap::new();
            let chars = unsafe { service.characteristics() }.unwrap_or_default();
            for c in chars {
                unsafe {
                    // Create the mp entry we'll need to export.
                    let uuid = mac_extensions_cb::cbuuid_to_uuid(unsafe { &c.UUID() });
                    characteristics.insert(uuid, c);
                }
            }
            let peripheral_uuid =
                mac_extensions_cb::nsuuid_to_uuid(unsafe { &peripheral.identifier() });
            unsafe {
                let service_uuid = mac_extensions_cb::cbuuid_to_uuid(&service.UUID());
                self.send_event(PeripheralDelegateEvent::DiscoveredCharacteristics {
                    service_uuid,
                    characteristics,
                    error: error.map(|e| e.localizedDescription().to_string()),
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

            let mut descriptors = HashMap::new();
            let descs = unsafe { characteristic.descriptors() }.unwrap_or_default();
            for d in descs {
                let uuid = unsafe { mac_extensions_cb::cbuuid_to_uuid(&d.UUID()) };
                descriptors.insert(uuid, d);
            }
            let service = unsafe { characteristic.service() }.unwrap();
            let service_uuid = unsafe { mac_extensions_cb::cbuuid_to_uuid(&service.UUID()) };
            let characteristic_uuid =
                unsafe { mac_extensions_cb::cbuuid_to_uuid(&characteristic.UUID()) };
            self.send_event(
                PeripheralDelegateEvent::DiscoveredCharacteristicDescriptors {
                    service_uuid,
                    characteristic_uuid,
                    descriptors,
                    error: error.map(|e| e.localizedDescription().to_string()),
                },
            );
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

            // TODO: make this a oneshot and pull logic into peripheral
            if error.is_none() {
                let service = unsafe { characteristic.service() }.unwrap();
                self.send_event(PeripheralDelegateEvent::CharacteristicNotified {
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

            // TODO: make this a oneshot and pull logic into peripheral
            if error.is_none() {
                let service = unsafe { characteristic.service() }.unwrap();
                self.send_event(PeripheralDelegateEvent::CharacteristicWritten {
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
            //
            // TODO: make this a oneshot and pull logic into peripheral
            let peripheral_uuid = nsuuid_to_uuid(unsafe { &peripheral.identifier() });
            let service = unsafe { characteristic.service() }.unwrap();
            let service_uuid = cbuuid_to_uuid(unsafe { &service.UUID() });
            let characteristic_uuid = cbuuid_to_uuid(unsafe { &characteristic.UUID() });
            if unsafe { characteristic.isNotifying() } {
                self.send_event(PeripheralDelegateEvent::CharacteristicSubscribed {
                    peripheral_uuid,
                    service_uuid,
                    characteristic_uuid,
                });
            } else {
                self.send_event(PeripheralDelegateEvent::CharacteristicUnsubscribed {
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

            // TODO: make this a oneshot and pull logic into peripheral
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

            // TODO: make this a oneshot and pull logic into peripheral
            if error.is_none() {
                let characteristic = unsafe { descriptor.characteristic() }.unwrap();
                let service = unsafe { characteristic.service() }.unwrap();
                self.send_event(PeripheralDelegateEvent::DescriptorNotified {
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

            // TODO: make this a oneshot and pull logic into peripheral
            if error.is_none() {
                let characteristic = unsafe { descriptor.characteristic() }.unwrap();
                let service = unsafe { characteristic.service() }.unwrap();
                self.send_event(PeripheralDelegateEvent::DescriptorWritten {
                    peripheral_uuid: nsuuid_to_uuid(unsafe { &peripheral.identifier() }),
                    service_uuid: cbuuid_to_uuid(unsafe { &service.UUID() }),
                    characteristic_uuid: cbuuid_to_uuid(unsafe { &characteristic.UUID() }),
                    descriptor_uuid: cbuuid_to_uuid(unsafe { &descriptor.UUID() }),
                });
            }
        }
    }
);

impl PeripheralDelegate {
    pub fn new(sender: Sender<PeripheralDelegateEvent>) -> Retained<PeripheralDelegate> {
        let this = PeripheralDelegate::alloc().set_ivars(IVars {
            sender,
            services_resolver: Arc::new(Mutex::new(ServiceResolver::new())),
            advertisement_resolver: Arc::new(Mutex::new(AdvertisementResolver::new())),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn send_event(&self, event: PeripheralDelegateEvent) {
        let sender = self.ivars().sender.clone();
        executor::block_on(async {
            if let Err(e) = sender.send(event).await {
                log::error!("Error sending delegate event: {}", e);
            }
        });
    }
}

pub enum PeripheralDelegateEvent {
    DiscoveredServices {
        services: HashMap<Uuid, Retained<CBService>>,
        error: Option<String>,
    },
    DiscoveredCharacteristics {
        service_uuid: Uuid,
        characteristics: HashMap<Uuid, Retained<CBCharacteristic>>,
        error: Option<String>,
    },
    DiscoveredCharacteristicDescriptors {
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        descriptors: HashMap<Uuid, Retained<CBDescriptor>>,
        error: Option<String>,
    },
    CharacteristicSubscribed {
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        error: Option<String>,
    },
    CharacteristicUnsubscribed {
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        error: Option<String>,
    },
    CharacteristicNotified {
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        characteristic: Retained<CBCharacteristic>,
        error: Option<String>,
    },
    CharacteristicWritten {
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        characteristic: Retained<CBCharacteristic>,
        error: Option<String>,
    },
    DescriptorNotified {
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        descriptor_uuid: Uuid,
        descriptor: Retained<CBDescriptor>,
        error: Option<String>,
    },
    DescriptorWritten {
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        descriptor_uuid: Uuid,
        descriptor: Retained<CBDescriptor>,
        error: Option<String>,
    },
}
