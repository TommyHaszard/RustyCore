use crate::api::peripheral::PeripheralManager;


pub struct Peripheral {
    manager_tx: Sender<ManagerEvent>,
}

#[async_trait]
impl PeripheralManager for Peripheral {

}

