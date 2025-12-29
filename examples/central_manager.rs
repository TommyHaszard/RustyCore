use rustycore::api::{
    central::{CentralManager, ScanFilter},
    central_event::CentralEvent,
};
use tokio::sync::mpsc;
use log::LevelFilter;


#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(LevelFilter::Info)
        .init();

    setup_central_manager().await;
}

async fn setup_central_manager() {
    let (sender_tx, mut receiver_rx) = mpsc::channel::<CentralEvent>(256);

    let mut central_manager = CentralManager::new(sender_tx).await.unwrap();
    
    // start scanning for devices
    central_manager.start_scan(ScanFilter::default()).await.unwrap();
    // Handle Updates
        while let Some(event) = receiver_rx.recv().await {
            handle_updates(event);
        }
    log::info!("Log");
}

/// Listen to all updates and respond if require
pub fn handle_updates(update: CentralEvent) {
    match update {
        _ => todo!()
    }
}
