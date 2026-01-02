use std::collections::HashMap;

use objc2::rc::Retained;
use objc2_core_bluetooth::{CBCharacteristic, CBDescriptor, CBPeripheral};
use tokio::sync::mpsc::{Receiver, Sender};
use uuid::Uuid;

use crate::corebluetooth::central_manager::PeripheralRemoteCommand;


struct Peripheral {
    peripheral: Retained<CBPeripheral>,
    services: HashMap<Uuid, ServiceInternal>,
    event_sender: Sender<PeripheralEventInternal>,
    manager_command_rx: Receiver<PeripheralRemoteCommand>,
}

impl Peripheral {
    pub fn new(
        peripheral: Retained<CBPeripheral>,
        event_sender: Sender<PeripheralEventInternal>,
    ) -> Self {
        Self {
            peripheral,
            services: HashMap::new(),
            event_sender,
        }
    }

    pub fn set_characteristics(
        &mut self,
        service_uuid: Uuid,
        characteristics: HashMap<Uuid, Retained<CBCharacteristic>>,
    ) {
        let characteristics = characteristics.into_iter().fold(
            // Only consider the first characteristic of each UUID
            // This "should" be unique, but of course it's not enforced
            HashMap::<Uuid, CharacteristicInternal>::new(),
            |mut map, (characteristic_uuid, characteristic)| {
                if !map.contains_key(&characteristic_uuid) {
                    map.insert(
                        characteristic_uuid,
                        CharacteristicInternal::new(characteristic),
                    );
                }
                map
            },
        );
        let service = self
            .services
            .get_mut(&service_uuid)
            .expect("Got characteristics for a service we don't know about");
        service.characteristics = characteristics;
        if service.characteristics.is_empty() {
            service.discovered = true;
            self.check_discovered();
        }
    }

    pub fn set_characteristic_descriptors(
        &mut self,
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        descriptors: HashMap<Uuid, Retained<CBDescriptor>>,
    ) {
        let descriptors = descriptors
            .into_iter()
            .map(|(descriptor_uuid, descriptor)| {
                (descriptor_uuid, DescriptorInternal::new(descriptor))
            })
            .collect();
        let service = self
            .services
            .get_mut(&service_uuid)
            .expect("Got descriptors for a service we don't know about");
        let characteristic = service
            .characteristics
            .get_mut(&characteristic_uuid)
            .expect("Got descriptors for a characteristic we don't know about");
        characteristic.descriptors = descriptors;
        characteristic.discovered = true;

        if !service
            .characteristics
            .values()
            .any(|characteristic| !characteristic.discovered)
        {
            service.discovered = true;
            self.check_discovered()
        }
    }

    fn check_discovered(&mut self) {
        // It's time for QUESTIONABLE ASSUMPTIONS.
        //
        // For sake of being lazy, we don't want to fire device connection until
        // we have all of our services and characteristics. We assume that
        // set_characteristics should be called once for every entry in the
        // service map. Once that's done, we're filled out enough and can send
        // back a Connected reply to the waiting future with all of the
        // characteristic info in it.
        if !self.services.values().any(|service| !service.discovered) {
            if self.connected_future_state.is_none() {
                panic!("We should still have a future at this point!");
            }
            let services = self
                .services
                .iter()
                .map(|(&service_uuid, service)| Service {
                    uuid: service_uuid,
                    primary: unsafe { service.cbservice.isPrimary() },
                    characteristics: service
                        .characteristics
                        .iter()
                        .map(|(&characteristic_uuid, characteristic)| {
                            let descriptors = characteristic
                                .descriptors
                                .iter()
                                .map(|(&descriptor_uuid, _)| Descriptor {
                                    uuid: descriptor_uuid,
                                    service_uuid,
                                    characteristic_uuid,
                                })
                                .collect();
                            Characteristic {
                                uuid: characteristic_uuid,
                                service_uuid,
                                descriptors,
                                properties: characteristic.properties,
                            }
                        })
                        .collect(),
                })
                .collect();
            self.connected_future_state
                .take()
                .unwrap()
                .lock()
                .unwrap()
                .set_reply(CoreBluetoothReply::Connected(services));
        }
    }

    pub fn confirm_disconnect(&mut self) {
        // Fulfill the disconnected future, if there is one.
        // There might not be a future if the device disconnects unexpectedly.
        if let Some(future) = self.disconnected_future_state.take() {
            future.lock().unwrap().set_reply(CoreBluetoothReply::Ok)
        }

        // Fulfill all pending futures
        let error = CoreBluetoothReply::Err(String::from("Device disconnected"));
        self.services.iter().for_each(|(_, service)| {
            service
                .characteristics
                .iter()
                .for_each(|(_, characteristic)| {
                    let CharacteristicInternal {
                        read_future_state,
                        write_future_state,
                        subscribe_future_state,
                        unsubscribe_future_state,
                        ..
                    } = characteristic;

                    let futures = read_future_state
                        .into_iter()
                        .chain(write_future_state.into_iter())
                        .chain(subscribe_future_state.into_iter())
                        .chain(unsubscribe_future_state.into_iter());
                    for state in futures {
                        state.lock().unwrap().set_reply(error.clone());
                    }
                });
        });
    }
}
