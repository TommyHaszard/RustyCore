use std::collections::HashMap;

use objc2::{msg_send, rc::Retained};
use objc2_core_bluetooth::{CBCharacteristic, CBDescriptor, CBPeripheral, CBService};
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot,
};
use uuid::Uuid;

use crate::{
    api::{
        central_event::CentralEvent, characteristic::Characteristic, descriptor::Descriptor,
        service::Service,
    },
    corebluetooth::{
        central_manager::PeripheralRemoteCommand,
        objc_bindings::peripheral_delegate_cb::{PeripheralDelegate, PeripheralDelegateEvent},
    },
};

struct Peripheral {
    peripheral: Retained<CBPeripheral>,
    delegate: Retained<PeripheralDelegate>,
    central_tx: Sender<CentralEvent>,
    cached_services: HashMap<Uuid, Retained<CBService>>,
    cached_characteristics: HashMap<Uuid, Retained<CBCharacteristic>>,
    cached_descriptors: HashMap<Uuid, Retained<CBDescriptor>>,
    remote_command_rx: Receiver<PeripheralRemoteCommand>,
    corebluetooth_delegate_rx: Receiver<PeripheralDelegateEvent>,
    service_discovery_resolver: Option<oneshot::Sender<Result<Vec<Service>, String>>>,
    characteristic_discovery_resolver:
        HashMap<Uuid, oneshot::Sender<Result<Vec<Characteristic>, String>>>,
    descriptor_discovery_resolver:
        HashMap<(Uuid, Uuid), oneshot::Sender<Result<Vec<Descriptor>, String>>>,
    read_resolver: HashMap<Uuid, oneshot::Sender<Result<Vec<u8>, String>>>,
    write_resolver: HashMap<Uuid, oneshot::Sender<Result<(), String>>>,
    subscribe_resolver: HashMap<Uuid, oneshot::Sender<Result<(), String>>>,
}

impl Peripheral {
    pub fn new(
        peripheral: Retained<CBPeripheral>,
        central_tx: Sender<CentralEvent>,
        remote_command_rx: Receiver<PeripheralRemoteCommand>,
    ) -> Self {
        let (delegate_tx, delegate_rx) = mpsc::channel::<PeripheralDelegateEvent>(256);

        let delegate: Retained<PeripheralDelegate> = PeripheralDelegate::new(delegate_tx);

        // attach this Rust instance with the Delegate in objc2 runtime
        unsafe {
            msg_send![&peripheral, setDelegate: &*delegate];
        }

        Self {
            peripheral,
            delegate,
            central_tx,
            remote_command_rx,
            corebluetooth_delegate_rx: delegate_rx,
            cached_services: HashMap::new(),
            cached_characteristics: HashMap::new(),
            cached_descriptors: HashMap::new(),
            service_discovery_resolver: None,
            characteristic_discovery_resolver: HashMap::new(),
            descriptor_discovery_resolver: HashMap::new(),
            read_resolver: HashMap::new(),
            write_resolver: HashMap::new(),
            subscribe_resolver: HashMap::new(),
        }
    }

    async fn handle_event(&mut self) {
        tokio::select! {
        // Match events from above
        Some(manager_command) = self.remote_command_rx.recv() => {
            match manager_command {
                PeripheralRemoteCommand::ConnectDevice { peripheral_uuid, responder } => todo!(),
                PeripheralRemoteCommand::DisconnectDevice { peripheral_uuid, responder } => todo!(),
                PeripheralRemoteCommand::ReadCharacteristicValue { peripheral_uuid, service_uuid, characteristic_uuid, responder } => todo!(),
                PeripheralRemoteCommand::WriteCharacteristicValue { peripheral_uuid, service_uuid, characteristic_uuid, data, write_type, responder } => todo!(),
                PeripheralRemoteCommand::SubscribeCharacteristic { peripheral_uuid, service_uuid, characteristic_uuid, responder } => todo!(),
                PeripheralRemoteCommand::UnsubscribeCharacteristic { peripheral_uuid, service_uuid, characteristic_uuid, responder } => todo!(),
                PeripheralRemoteCommand::IsConnected { peripheral_uuid, responder } => todo!(),
                PeripheralRemoteCommand::ReadDescriptorValue { peripheral_uuid, service_uuid, characteristic_uuid, descriptor_uuid, responder } => todo!(),
                PeripheralRemoteCommand::WriteDescriptorValue { peripheral_uuid, service_uuid, characteristic_uuid, descriptor_uuid, data, responder } => todo!(),
            }
        }

        // Match events from Corebluetooth delegate
        Some(delegate_event) = self.corebluetooth_delegate_rx.recv() => {
            match delegate_event {
                PeripheralDelegateEvent::DiscoveredServices { services, error } => self.discovered_services(services, error),
                PeripheralDelegateEvent::DiscoveredCharacteristics { service_uuid, characteristics, error } => self.discovered_characteristics(service_uuid, characteristics, error),
                PeripheralDelegateEvent::DiscoveredCharacteristicDescriptors { service_uuid, characteristic_uuid, descriptors, error } => self.discovered_descriptors(service_uuid, characteristic_uuid, descriptors, error),
                PeripheralDelegateEvent::CharacteristicSubscribed {  service_uuid, characteristic_uuid, error} => todo!(),
                PeripheralDelegateEvent::CharacteristicUnsubscribed {  service_uuid, characteristic_uuid, error} => todo!(),
                PeripheralDelegateEvent::CharacteristicNotified {  service_uuid, characteristic_uuid, characteristic, error} => todo!(),
                PeripheralDelegateEvent::CharacteristicWritten {  service_uuid, characteristic_uuid, characteristic, error } => todo!(),
                PeripheralDelegateEvent::DescriptorNotified {  service_uuid, characteristic_uuid, descriptor_uuid, descriptor, error } => todo!(),
                PeripheralDelegateEvent::DescriptorWritten {  service_uuid, characteristic_uuid, descriptor_uuid, descriptor, error } => todo!(),
            }
            }
        };
    }

    // NOTE: We auto discover services when the Delegate discovered_peripheral is triggered.
    // Don't return the Service until we have finished discovering all the Characteristics and
    // Descriptors
    fn discovered_services(
        &mut self,
        services: HashMap<Uuid, Retained<CBService>>,
        error: Option<String>,
    ) {
        for service in services.values() {
            unsafe {
                self.peripheral
                    .discoverCharacteristics_forService(None, &service);
                self.peripheral
                    .discoverIncludedServices_forService(None, &service);
            }
        }
        self.cached_services.extend(services);
    }

    // NOTE: We auto discover characteristics when the Delegate
    // didDiscoverCharacteristicsForService is triggered.
    // Don't return the Service until we have finished discovering all the Characteristics and
    // Descriptors
    fn discovered_characteristics(
        &mut self,
        service: Uuid,
        characteristics: HashMap<Uuid, Retained<CBCharacteristic>>,
        error: Option<String>,
    ) {
        for characteristic in characteristics.values() {
            unsafe {
                self.peripheral
                    .discoverDescriptorsForCharacteristic(&characteristic)
            };
        }
        self.cached_characteristics.extend(characteristics);
    }

    // NOTE: We auto discover descriptors when the Delegate
    // didDiscoverDescriptorsForCharacteristic is triggered.
    // Don't return the Service until we have finished discovering all the Characteristics and
    // Descriptors.
    fn discovered_descriptors(
        &mut self,
        service: Uuid,
        characteristic_uuid: Uuid,
        descriptors: HashMap<Uuid, Retained<CBDescriptor>>,
        error: Option<String>,
    ) {
        self.cached_descriptors.extend(descriptors);
    }

    pub fn update_cached_characteristics(
        &mut self,
        service_uuid: Uuid,
        value: HashMap<Uuid, Retained<CBCharacteristic>>,
    ) {
    }

    pub fn update_cached_characteristic_descriptors(
        &mut self,
        service_uuid: Uuid,
        characteristic_uuid: Uuid,
        descriptors: HashMap<Uuid, Retained<CBDescriptor>>,
    ) {
    }

    fn check_discovered(&mut self, service_uuid: Uuid) {
        // It's time for QUESTIONABLE ASSUMPTIONS.
        //
        // For sake of being lazy, we don't want to fire device connection until
        // we have all of our services and characteristics. We assume that
        // set_characteristics should be called once for every entry in the
        // service map. Once that's done, we're filled out enough and can send
        // back a Connected reply to the waiting future with all of the
        // characteristic info in it.
        if self.delegate.is_waiting_for_service(&service_uuid) {
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
    }

    pub fn confirm_disconnect(&mut self) {
        // Fulfill the disconnected future, if there is one.
        // There might not be a future if the device disconnects unexpectedly.
        if let Some(future) = self.disconnected_future_state.take() {
            future.lock().unwrap().set_reply(CoreBluetoothReply::Ok)
        }
        self.peripheral.readValueForDescriptor(descriptor);

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
