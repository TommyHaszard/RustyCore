use super::mac_utils_cb;
use super::{characteristic_utils_cb::parse_characteristic, mac_extensions_cb::uuid_to_cbuuid};
use crate::Error;
use crate::api::central::{PeripheralRemote, ScanFilter};
use crate::api::characteristic::CharacteristicWriteType;
use crate::corebluetooth::central_manager::{CentralManagerCommand, Peripheral};
use crate::corebluetooth::objc_bindings::central_delegate_cb::CentralDelegate;
use objc2::{AnyThread, msg_send};
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_core_bluetooth::{
    CBAdvertisementDataLocalNameKey, CBAdvertisementDataServiceUUIDsKey, CBCentralManager, CBCharacteristic, CBManager, CBManagerAuthorization, CBManagerState, CBMutableCharacteristic, CBMutableService, CBPeripheralManager
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
use tokio::sync::oneshot;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::OnceLock;
use std::thread;
use tokio::runtime;
use tokio::sync::{
    mpsc::{Sender,Receiver},
};
use uuid::Uuid;

use crate::api::central_event::CentralEvent;

static CENTRAL_THREAD: OnceLock<()> = OnceLock::new();

// Handle Peripheral Manager and all communication in a separate thread
pub fn run_central_thread(sender: Sender<CentralEvent>, listener: Receiver<CentralManagerCommand>) {
    CENTRAL_THREAD.get_or_init(|| {
        thread::spawn(move || {
            let runtime = runtime::Builder::new_current_thread().enable_time().build();
            if runtime.is_err() {
                log::error!("Failed to create runtime");
                return;
            }
            runtime.unwrap().block_on(async move {
                let mut central_manager = CentralManager::new(sender, listener);
                loop {
                    central_manager.handle_event().await;
                }
            })
        });
    });
}

struct CentralManager {
    manager: Retained<CBCentralManager>,
    delegate: Retained<CentralDelegate>,
    peripherals: HashMap<Uuid, Peripheral>,
    manager_command_rx: Receiver<CentralManagerCommand>,
}

impl CentralManager {
    fn new(central_tx: Sender<CentralEvent>, manager_rx: Receiver<CentralManagerCommand>) -> Self{
        let delegate: Retained<CentralDelegate> = CentralDelegate::new(central_tx);

        let label = CString::new("CBqueue").unwrap();
        let queue =
            unsafe { mac_utils_cb::dispatch_queue_create(label.as_ptr(), mac_utils_cb::DISPATCH_QUEUE_SERIAL) };
        let queue: *mut AnyObject = queue.cast();

        let manager: Retained<CBCentralManager> = unsafe {
            msg_send![CBCentralManager::alloc(), initWithDelegate: &*delegate, queue: queue]
        };

        Self {
            manager,
            delegate,
            peripherals: HashMap::new(),
            manager_command_rx: manager_rx,
        }
    }

    async fn handle_event(&mut self) {
        if let Some(event) = self.manager_command_rx.recv().await {
            let _ = match event {
                CentralManagerCommand::GetAdapterState { responder } => todo!(),
                CentralManagerCommand::StartScanning { filter } => todo!(),
                CentralManagerCommand::StopScanning => todo!(),
            };
        }
    }
}
