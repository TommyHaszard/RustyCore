use std::collections::HashMap;

use tokio::sync::oneshot;
use uuid::Uuid;

mod central_manager_delegate_cb;
pub mod central_manager_cb;
mod characteristic_utils_cb;
mod error_cb;
mod mac_extensions_cb;
mod mac_utils_cb;
mod peripheral_manager_delegate_cb;
pub mod peripheral_manager_cb;
pub mod peripheral_delegate_cb;
mod peripheral_cb;

#[derive(Debug)]
pub struct ServiceResolver(HashMap<Uuid, oneshot::Sender<Option<String>>>);


#[derive(Debug)]
pub struct AdvertisementResolver(Option<oneshot::Sender<Option<String>>>);


impl ServiceResolver {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn is_waiting_for(&self, service_uuid: &Uuid) -> bool {
        self.0.contains_key(service_uuid)
    }

    pub fn has_pending(&self) -> bool {
        !self.0.is_empty()
    }

    pub fn register(&mut self, service_uuid: Uuid, sender: oneshot::Sender<Option<String>>) {
        self.0.insert(service_uuid, sender);
    }

     pub fn take(&mut self, service_uuid: &Uuid) -> Option<oneshot::Sender<Option<String>>> {
        self.0.remove(service_uuid)
    }

    pub fn cancel(&mut self, service_uuid: &Uuid) -> bool {
        self.0.remove(service_uuid).is_some()
    }

    pub fn count(&self) -> usize {
        self.0.len()
    }
}

impl Default for ServiceResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvertisementResolver {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn is_waiting(&self) -> bool {
        self.0.is_some()
    }

    pub fn register(&mut self, sender: oneshot::Sender<Option<String>>) {
        self.0 = Some(sender);
    }

    pub fn take(&mut self) -> Option<oneshot::Sender<Option<String>>> {
        self.0.take()
    }

    pub fn cancel(&mut self) {
        self.0 = None;
    }
}
