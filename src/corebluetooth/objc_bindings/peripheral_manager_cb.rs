use tokio::sync::{oneshot};
use super::mac_utils_cb;
use super::peripheral_delegate_cb::PeripheralDelegate;
use super::{characteristic_utils_cb::parse_characteristic, mac_extensions_cb::uuid_to_cbuuid};
use crate::Error;
use crate::api::peripheral_event::PeripheralEvent;
use crate::api::service::Service;
use crate::corebluetooth::peripheral_manager::PeripheralManagerCommand;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2::{AnyThread, msg_send};
use objc2_core_bluetooth::{
    CBAdvertisementDataLocalNameKey, CBAdvertisementDataServiceUUIDsKey, CBCharacteristic,
    CBManager, CBManagerAuthorization, CBManagerState, CBMutableCharacteristic, CBMutableService,
    CBPeripheralManager,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::OnceLock;
use std::thread;
use tokio::runtime;
use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;


static PERIPHERAL_THREAD: OnceLock<()> = OnceLock::new();

// Handle Peripheral Manager and all communication in a separate thread
pub fn run_peripheral_thread(sender: Sender<PeripheralEvent>, listener: Receiver<PeripheralManagerCommand>) {
    PERIPHERAL_THREAD.get_or_init(|| {
        thread::spawn(move || {
            let runtime = runtime::Builder::new_current_thread().enable_time().build();
            if runtime.is_err() {
                log::error!("Failed to create runtime");
                return;
            }
            runtime.unwrap().block_on(async move {
                let mut peripheral_manager = PeripheralManager::new(sender, listener);
                loop {
                    peripheral_manager.handle_event().await;
                }
            })
        });
    });
}

#[derive(Debug)]
struct PeripheralManager {
    manager_command_rx: Receiver<PeripheralManagerCommand>,
    cb_peripheral_manager: Retained<CBPeripheralManager>,
    peripheral_delegate: Retained<PeripheralDelegate>,
    cached_characteristics: HashMap<Uuid, Retained<CBMutableCharacteristic>>,
}

impl PeripheralManager {
    fn new(peripheral_tx: Sender<PeripheralEvent>, manager_rx: Receiver<PeripheralManagerCommand>) -> Self {
        let delegate: Retained<PeripheralDelegate> = PeripheralDelegate::new(peripheral_tx);
        let label: CString = CString::new("CBqueue").unwrap();
        let queue: *mut std::ffi::c_void = unsafe {
            mac_utils_cb::dispatch_queue_create(label.as_ptr(), mac_utils_cb::DISPATCH_QUEUE_SERIAL)
        };
        let queue: *mut AnyObject = queue.cast();
        let peripheral_manager: Retained<CBPeripheralManager> = unsafe {
            msg_send![CBPeripheralManager::alloc(), initWithDelegate: &**delegate, queue: queue]
        };

        Self {
            manager_command_rx: manager_rx,
            cb_peripheral_manager: peripheral_manager,
            peripheral_delegate: delegate,
            cached_characteristics: HashMap::new(),
        }
    }

    async fn handle_event(&mut self) {
        if let Some(event) = self.manager_command_rx.recv().await {
            let _ = match event {
                PeripheralManagerCommand::IsPowered { responder } => {
                    let _ = responder.send(Ok(self.is_powered()));
                }
                PeripheralManagerCommand::IsAdvertising { responder } => {
                    let _ = responder.send(Ok(self.is_advertising()));
                }
                PeripheralManagerCommand::StartAdvertising {
                    name,
                    uuids,
                    responder,
                } => {
                    let _ = responder.send(self.start_advertising(&name, &uuids).await);
                }
                PeripheralManagerCommand::StopAdvertising { responder } => {
                    let _ = responder.send(Ok(self.stop_advertising()));
                }
                PeripheralManagerCommand::AddService { service, responder } => {
                    let _ = responder.send(self.add_service(&service).await);
                }
                PeripheralManagerCommand::UpdateCharacteristic {
                    characteristic,
                    value,
                    responder,
                } => {
                    let _ = responder.send(self.update_characteristic(characteristic, value).await);
                }
            };
        }
    }

    fn is_powered(self: &Self) -> bool {
        unsafe {
            let state = self.cb_peripheral_manager.state();
            state == CBManagerState::PoweredOn
        }
    }

    async fn start_advertising(self: &Self, name: &str, uuids: &[Uuid]) -> Result<(), Error> {
        if self
            .peripheral_delegate
            .is_waiting_for_advertisement_result()
        {
            return Err(Error::from_string(
                "Already in progress".to_string(),
            ));
        }

        let mut keys: Vec<&NSString> = vec![];
        let mut objects: Vec<Retained<AnyObject>> = vec![];

        unsafe {
            keys.push(CBAdvertisementDataLocalNameKey);
            objects.push(Retained::cast_unchecked(NSString::from_str(name)));

            keys.push(CBAdvertisementDataServiceUUIDsKey);
            objects.push(Retained::cast_unchecked(NSArray::from_retained_slice(
                uuids.iter().map(|u| uuid_to_cbuuid(u.clone())).collect(),
            )));
        }

        let advertising_data: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_retained_objects(&keys, &objects);

        unsafe {
            self.cb_peripheral_manager
                .startAdvertising(Some(&advertising_data));
        }

        return self
            .peripheral_delegate
            .ensure_advertisement_started()
            .await;
    }

    fn stop_advertising(self: &Self) {
        unsafe {
            self.cb_peripheral_manager.stopAdvertising();
        }
    }

    fn is_advertising(self: &Self) -> bool {
        unsafe { self.cb_peripheral_manager.isAdvertising() }
    }

    async fn update_characteristic(
        &mut self,
        characteristic: Uuid,
        value: Vec<u8>,
    ) -> Result<(), Error> {
        if let Some(char) = self.cached_characteristics.get(&characteristic) {
            unsafe {
                self.cb_peripheral_manager
                    .updateValue_forCharacteristic_onSubscribedCentrals(
                        &NSData::from_vec(value.clone()),
                        char,
                        None,
                    );
            }
        }
        return Ok(());
    }

    // Peripheral with cache value must only have Read permission, else it will crash
    // TODO: throw proper error, or catch Objc errors
    async fn add_service(&mut self, service: &Service) -> Result<(), Error> {
        if self
            .peripheral_delegate
            .is_waiting_for_service_result(service.uuid)
        {
            return Err(Error::from_string(
                "Already in progress".to_string(),
            ));
        }

        unsafe {
            let mut characteristics: Vec<Retained<CBCharacteristic>> = Vec::new();

            for char in service.characteristics.iter() {
                let cb_char = parse_characteristic(char);
                characteristics.push(Retained::into_super(cb_char.clone()));
                self.cached_characteristics.insert(char.uuid, cb_char);
            }

            let mutable_service: Retained<CBMutableService> =
                CBMutableService::initWithType_primary(
                    CBMutableService::alloc(),
                    &uuid_to_cbuuid(service.uuid),
                    service.primary,
                );

            if !characteristics.is_empty() {
                let chars = NSArray::from_retained_slice(&characteristics);
                mutable_service.setCharacteristics(Some(&chars));
            }

            self.cb_peripheral_manager.addService(&mutable_service);

            return self
                .peripheral_delegate
                .ensure_service_added(service.uuid)
                .await;
        }
    }
}

pub fn is_authorized() -> bool {
    let authorization = unsafe { CBManager::authorization_class() };
    return authorization != CBManagerAuthorization::Restricted
        && authorization != CBManagerAuthorization::Denied;
}

