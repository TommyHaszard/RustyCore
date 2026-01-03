use super::mac_utils_cb;
use crate::corebluetooth::central_manager::{CentralManagerCommand, Peripheral};
use crate::corebluetooth::objc_bindings::central_manager_delegate_cb::{
    CentralManagerDelegate, CentralManagerDelegateEvent,
};
use objc2::{AnyThread, msg_send};
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_core_bluetooth::CBCentralManager;
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::OnceLock;
use std::thread;
use tokio::runtime;
use tokio::sync::mpsc::{self, Receiver, Sender};
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
    delegate: Retained<CentralManagerDelegate>,
    peripherals: HashMap<Uuid, Peripheral>,
    manager_command_rx: Receiver<CentralManagerCommand>,
    corebluetooth_delegate_rx: Receiver<CentralManagerDelegateEvent>,
    central_tx: Sender<CentralEvent>,
}

impl CentralManager {
    fn new(central_tx: Sender<CentralEvent>, manager_rx: Receiver<CentralManagerCommand>) -> Self {
        let (delegate_tx, delegate_rx) = mpsc::channel::<CentralManagerDelegateEvent>(256);

        let delegate: Retained<CentralManagerDelegate> = CentralManagerDelegate::new(delegate_tx);

        let label = CString::new("CBqueue").unwrap();
        let queue = unsafe {
            mac_utils_cb::dispatch_queue_create(label.as_ptr(), mac_utils_cb::DISPATCH_QUEUE_SERIAL)
        };
        let queue: *mut AnyObject = queue.cast();

        let manager: Retained<CBCentralManager> = unsafe {
            msg_send![CBCentralManager::alloc(), initWithDelegate: &*delegate, queue: queue]
        };

        Self {
            manager,
            delegate,
            peripherals: HashMap::new(),
            manager_command_rx: manager_rx,
            corebluetooth_delegate_rx: delegate_rx,
            central_tx,
        }
    }

    async fn handle_event(&mut self) {
        tokio::select! {
            // Match events from above
            Some(manager_command) = self.manager_command_rx.recv() => {
                match manager_command {
                    CentralManagerCommand::GetAdapterState { responder } => todo!(),
                    CentralManagerCommand::StartScanning { filter } => todo!(),
                    CentralManagerCommand::StopScanning => todo!(),
                }
            }

            // Match events from Corebluetooth delegate
            Some(delegate_event) = self.corebluetooth_delegate_rx.recv() => {
                match delegate_event {
                }
            }
        };
    }
}
