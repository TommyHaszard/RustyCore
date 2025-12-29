use rustycore::api::{
    central::{CentralManager, ScanFilter},
    central_event::CentralEvent, peripheral_event::PeripheralEvent,
};
use tokio::sync::mpsc;
use log::LevelFilter;


#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(LevelFilter::Info)
        .init();

     
    let (sender_tx, mut receiver_rx) = mpsc::channel::<CustomEventEnum>(256);
    setup_central_manager(sender_tx.clone()).await;
    setup_peripheral_manager(sender_tx).await;
}

async fn setup_central_manager(api_event_tx: Sender<CustomEventEnum>) {
    let (sender_tx, mut receiver_rx) = mpsc::channel::<CentralEvent>(256);

    let mut central_manager = CentralManager::new(sender_tx).await.unwrap();
    
    // start scanning for devices
    central_manager.start_scan(ScanFilter::default()).await.unwrap();
    // Handle Updates
    tokio::spawn(async move {
        while let Some(event) = receiver_rx.recv().await {
            handle_central_updates(event, api_event_tx);
        }
    });
    log::info!("Log");
}

async fn setup_peripheral_manager(api_event_tx: Sender<CustomEventEnum>) {
    let (sender_tx, mut receiver_rx) = mpsc::channel::<PeripheralEvent>(256);

    let mut peripheral_manager = PeripheralManager::new(sender_tx).await.unwrap();
    
    // start advertising for centrals to connect
    peripheral_manager.start_advertising("user_name", );
    
    // Handle Updates
    tokio::spawn(async move {
        while let Some(event) = receiver_rx.recv().await {
            handle_peripheral_updates(event, api_event_tx);
        }
    });
    log::info!("Log");
}

/// Listen to all updates and respond if require
pub fn handle_central_updates(update: CentralEvent, tx: Sender<CustomEventEnum>) {
    match update {
        _ => todo!()
    }
}

/// Listen to all updates and respond if require
pub fn handle_peripheral_updates(update: PeripheralEvent, tx: Sender<CustomEventEnum>) {
    match update {
        _ => todo!()
    }
}

enum CustomEventEnum {

}
