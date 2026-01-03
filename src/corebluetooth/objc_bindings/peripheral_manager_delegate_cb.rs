use super::mac_extensions_cb::UuidExtension;
use crate::{
    Error, ErrorType,
    api::peripheral_event::{
        PeripheralEvent, PeripheralRequest, ReadRequestResponse, RequestResponse,
        WriteRequestResponse,
    },
    corebluetooth::objc_bindings::{AdvertisementResolver, ServiceResolver},
};
use ::futures::executor;
use objc2::{AnyThread, DeclaredClass, define_class, msg_send, rc::Retained};
use objc2_core_bluetooth::{
    CBATTError, CBATTRequest, CBCentral, CBCharacteristic, CBManagerState, CBPeripheralManager,
    CBPeripheralManagerDelegate, CBService,
};
use objc2_foundation::{NSArray, NSData, NSError, NSObject, NSObjectProtocol};
use std::{
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc::Sender, oneshot};
use tokio::time::{Duration, timeout};
use uuid::Uuid;

// Instance Variables that are stored within the ObjC class allowing communication between Rust
// code and the ObjC class.
#[derive(Debug)]
pub struct IVars {
    pub sender: Sender<PeripheralManagerDelegateEvent>,
    pub services_resolver: Arc<Mutex<ServiceResolver>>,
    pub advertisement_resolver: Arc<Mutex<AdvertisementResolver>>,
}

// Macro for defining the ObjC class
define_class! {
    #[derive(Debug)]
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    #[name = "PeripheralManagerDelegate"]
    #[ivars = IVars]
    pub struct PeripheralManagerDelegate;

    unsafe impl NSObjectProtocol for PeripheralManagerDelegate {}

    unsafe impl CBPeripheralManagerDelegate for PeripheralManagerDelegate {
        #[unsafe(method(peripheralManagerDidUpdateState:))]
         fn delegate_peripheralmanagerdidupdatestate(&self, peripheral: &CBPeripheralManager){
                let state = unsafe { peripheral.state() };
                self.send_event(PeripheralEvent::StateUpdate { is_powered : state == CBManagerState::PoweredOn });
         }

        #[unsafe(method(peripheralManagerDidStartAdvertising:error:))]
        fn delegate_peripheralmanagerdidstartadvertising_error(&self, _: &CBPeripheralManager,error: Option<&NSError>){
            let mut error_desc: Option<String> = None;
            if let Some(error) = error {
                error_desc = Some(error.localizedDescription().to_string());
            }
            log::debug!("Advertising, Error: {error_desc:?}");
            if let Ok(mut resolver) = self.ivars().advertisement_resolver.lock() {
                let sender_opt = resolver.take();
                drop(resolver);
                if let Some(sender) = sender_opt {
                    let _ = sender.send(error_desc);
                }
            }
        }

        #[unsafe(method(peripheralManager:didAddService:error:))]
         fn delegate_peripheralmanager_didaddservice_error(&self, _: &CBPeripheralManager,service: &CBService, error: Option<&NSError>){
            let mut error_desc: Option<String> = None;
            if let Some(error) = error {
                error_desc = Some(error.localizedDescription().to_string());
            }
            log::debug!("AddServices, Error: {error_desc:?}");


            if let Ok(mut resolver) = self.ivars().services_resolver.lock() {
                if let Some(sender) = resolver.take(&service.get_uuid()) {
                    drop(resolver); // Explicit drop before send
                    let _ = sender.send(error_desc);
                }
            }
        }

        #[unsafe(method(peripheralManager:central:didSubscribeToCharacteristic:))]
         fn delegate_peripheralmanager_central_didsubscribetocharacteristic(
            &self,
            _: &CBPeripheralManager,
            central: &CBCentral,
            characteristic: &CBCharacteristic,
        ){
            unsafe{
                let service: Option<Retained<CBService>> = characteristic.service();
                if service.is_none() {
                    return;
                }
                self.send_event(PeripheralEvent::CharacteristicSubscriptionUpdate {
                    request: PeripheralRequest {
                        client: central.identifier().to_string(),
                        service: characteristic.service().unwrap().get_uuid(),
                        characteristic: characteristic.get_uuid(),
                    },
                    subscribed: true,
                });
            }
        }

        #[unsafe(method(peripheralManager:central:didUnsubscribeFromCharacteristic:))]
         fn delegate_peripheralmanager_central_didunsubscribefromcharacteristic(
            &self,
            _: &CBPeripheralManager,
            central: &CBCentral,
            characteristic: &CBCharacteristic,
        ){  unsafe{
            let service: Option<Retained<CBService>> = characteristic.service();
            if service.is_none() {
                return;
            }

            self.send_event(PeripheralEvent::CharacteristicSubscriptionUpdate {
               request: PeripheralRequest {
                    client: central.identifier().to_string(),
                    service: characteristic.service().unwrap().get_uuid(),
                    characteristic: characteristic.get_uuid(),
                },
                subscribed: false,
            });
        }}

        #[unsafe(method(peripheralManager:didReceiveReadRequest:))]
         fn delegate_peripheralmanager_didreceivereadrequest(
            &self,
            manager: &CBPeripheralManager,
            request: &CBATTRequest,
        ){
            unsafe{
                let service = request.characteristic().service();
                if service.is_none() {
                    return;
                }
                let central = request.central();
                let characteristic = request.characteristic();

                self.send_read_request(
                    PeripheralRequest{
                        client: central.identifier().to_string(),
                        service: characteristic.service().unwrap().get_uuid(),
                        characteristic: characteristic.get_uuid(),
                    },
                    manager,
                    request,
                );
            }
        }

        #[unsafe(method(peripheralManager:didReceiveWriteRequests:))]
         fn delegate_peripheralmanager_didreceivewriterequests(
            &self,
            manager: &CBPeripheralManager,
            requests: &NSArray<CBATTRequest>,
        ){
            for request in requests {
                unsafe{
                    let service = request.characteristic().service();
                    if service.is_none() {
                        return;
                    }
                    let mut value: Vec<u8> = Vec::new();
                    if let Some(ns_data) = request.value() {
                       value = ns_data.as_bytes_unchecked().to_vec();
                    }
                    let central = request.central();
                    let characteristic = request.characteristic();

                    self.send_write_request(
                        PeripheralRequest{
                             client: central.identifier().to_string(),
                            service: characteristic.service().unwrap().get_uuid(),
                            characteristic: characteristic.get_uuid(),
                        },
                        manager,
                        &request,
                        value,
                    );
                }
            }
        }
    }
}

impl PeripheralManagerDelegate {
    pub fn new(sender: Sender<PeripheralManagerDelegateEvent>) -> Retained<PeripheralManagerDelegate> {
        let this = PeripheralManagerDelegate::alloc().set_ivars(IVars {
            sender,
            services_resolver: Arc::new(Mutex::new(ServiceResolver::new())),
            advertisement_resolver: Arc::new(Mutex::new(AdvertisementResolver::new()))
        });
        unsafe { msg_send![super(this), init] }
    }

    pub fn is_waiting_for_advertisement_result(&self) -> bool {
        if let Ok(resolver) = self.ivars().advertisement_resolver.lock() {
            return resolver.is_waiting();
        }
        return false;
    }

    /// Wait for delegate to ensure advertisement started successfully
    pub async fn ensure_advertisement_started(&self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel::<Option<String>>();
        {
            if let Ok(mut resolver) = self.ivars().advertisement_resolver.lock() {
                resolver.register(sender);
            }
        }

        let event = timeout(Duration::from_secs(5), receiver).await;

        {
            if let Ok(mut resolver) = self.ivars().advertisement_resolver.lock() {
                resolver.cancel();
            }
        }
        return self.resolve_event(event);
    }

    pub fn is_waiting_for_service_result(&self, service: Uuid) -> bool {
        if let Ok(resolver) = self.ivars().services_resolver.lock() {
            return resolver.is_waiting_for(&service);
        }
        return false;
    }

    // Wait for event from delegate if service added successfully
    pub async fn ensure_service_added(&self, service: Uuid) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel::<Option<String>>();
        {
            if let Ok(mut resolver) = self.ivars().services_resolver.lock() {
                resolver.register(service, sender);
            }
        }

        let event = timeout(Duration::from_secs(5), receiver).await;

        {
            if let Ok(mut resolver) = self.ivars().services_resolver.lock() {
                resolver.cancel(&service);
            }
        }

        return self.resolve_event(event);
    }

    fn resolve_event(
        &self,
        event: Result<
            Result<Option<String>, oneshot::error::RecvError>,
            tokio::time::error::Elapsed,
        >,
    ) -> Result<(), Error> {
        let event = match event {
            Ok(Ok(event)) => event,
            Ok(Err(e)) => {
                return Err(Error::from_string(
                    format!("Channel error while waiting: {}", e),
                    ErrorType::CoreBluetooth,
                ));
            }
            Err(_) => {
                return Err(Error::from_string(
                    "Timeout waiting for event".to_string(),
                    ErrorType::CoreBluetooth,
                ));
            }
        };

        if let Some(error) = event {
            return Err(Error::from_string(error, ErrorType::CoreBluetooth));
        }

        return Ok(());
    }
}

/// Event handler
impl PeripheralManagerDelegate {
    fn send_event(&self, event: PeripheralEvent) {
        let sender = self.ivars().sender.clone();
        executor::block_on(async {
            if let Err(e) = sender.send(event).await {
                log::error!("Error sending delegate event: {}", e);
            }
        });
    }

    fn send_read_request(
        &self,
        peripheral_request: PeripheralRequest,
        manager: &CBPeripheralManager,
        request: &CBATTRequest,
    ) {
        let sender = self.ivars().sender.clone();
        unsafe {
            executor::block_on(async {
                let (resp_tx, resp_rx) = oneshot::channel::<ReadRequestResponse>();

                if let Err(e) = sender
                    .send(PeripheralEvent::ReadRequest {
                        request: peripheral_request,
                        offset: request.offset() as u64,
                        responder: resp_tx,
                    })
                    .await
                {
                    log::error!("Error sending delegate event: {}", e);
                    return;
                }

                let mut cb_att_error = CBATTError::InvalidHandle;
                if let Ok(result) = resp_rx.await {
                    cb_att_error = result.response.to_cb_error();
                    request.setValue(Some(&NSData::from_vec(result.value)));
                }
                manager.respondToRequest_withResult(request, cb_att_error);
            });
        };
    }

    fn send_write_request(
        &self,
        peripheral_request: PeripheralRequest,
        manager: &CBPeripheralManager,
        request: &CBATTRequest,
        value: Vec<u8>,
    ) {
        let sender = self.ivars().sender.clone();
        unsafe {
            executor::block_on(async {
                let (resp_tx, resp_rx) = oneshot::channel::<WriteRequestResponse>();

                if let Err(e) = sender
                    .send(PeripheralEvent::WriteRequest {
                        request: peripheral_request,
                        value,
                        offset: request.offset() as u64,
                        responder: resp_tx,
                    })
                    .await
                {
                    log::error!("Error sending delegate event: {}", e);
                    return;
                }

                let mut cb_att_error = CBATTError::InvalidHandle;
                if let Ok(result) = resp_rx.await {
                    cb_att_error = result.response.to_cb_error();
                }

                manager.respondToRequest_withResult(request, cb_att_error);
            });
        };
    }
}

pub enum PeripheralManagerDelegateEvent {

} 

impl RequestResponse {
    fn to_cb_error(self) -> CBATTError {
        match self {
            RequestResponse::Success => CBATTError::Success,
            RequestResponse::InvalidHandle => CBATTError::InvalidHandle,
            RequestResponse::RequestNotSupported => CBATTError::RequestNotSupported,
            RequestResponse::InvalidOffset => CBATTError::InvalidOffset,
            RequestResponse::UnlikelyError => CBATTError::UnlikelyError,
        }
    }
}

