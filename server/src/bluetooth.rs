use bluer::{
    adv::{Advertisement, Type},
    Adapter, AdapterEvent, Address, DeviceEvent, DeviceProperty, DiscoveryFilter,
    DiscoveryTransport,
};
use codec::{Decode, Encode};
use futures::stream::StreamExt;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::{task, time};

const MAX_RSSI_QUEUE_SIZE: usize = 5;
const BLUETOOTH_SERVICE_UUID: &str = "0000b4e7-0000-1000-8000-00805f9b34fb";

#[derive(Encode, Decode, Debug, Clone)]
pub struct DeviceRssi {
    pub address: [u8; 6],
    pub rssi: i16,
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct RssiResponse {
    pub devices: Vec<DeviceRssi>,
}

// Global shared state for neighbor addresses
pub type NeighborAddresses = Arc<Mutex<HashSet<Address>>>;

/// Initialize neighbor addresses from environment variable (for backwards compatibility/testing)
pub fn init_neighbor_addresses_from_env() -> HashSet<Address> {
    std::env::var("BLUETOOTH_ADDRESSES")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

pub async fn bluetooth_address(adapter: &Adapter) -> Address {
    adapter
        .address()
        .await
        .expect("Failed to get adapter address")
}

fn calculate_median(values: &mut Vec<i16>) -> Option<i16> {
    if values.is_empty() {
        return None;
    }

    values.sort_unstable();
    let len = values.len();

    if len % 2 == 0 {
        Some((values[len / 2 - 1] + values[len / 2]) / 2)
    } else {
        Some(values[len / 2])
    }
}

// Global shared state for RSSI data
pub type RssiData = Arc<Mutex<HashMap<Address, VecDeque<i16>>>>;

async fn start_advertising(adapter: &Adapter) -> Result<(), Box<dyn Error>> {
    println!("Starting BLE advertising...");

    let advertisement = Advertisement {
        // If it never connects, it should be 'Broadcast'.
        advertisement_type: Type::Broadcast,

        // Add a service UUID. This is often used by apps to find specific devices.
        service_uuids: [BLUETOOTH_SERVICE_UUID.parse().unwrap()]
            .into_iter()
            .collect(),

        ..Default::default()
    };

    let _handle = adapter.advertise(advertisement).await?;
    println!(
        "BLE advertising started with service UUID: {}",
        BLUETOOTH_SERVICE_UUID
    );

    // Keep advertising running indefinitely
    loop {
        time::sleep(Duration::from_secs(60)).await;
    }
}

async fn scan_devices(
    adapter: &Adapter,
    rssi_data: RssiData,
    neighbor_addresses: NeighborAddresses,
) -> Result<(), Box<dyn Error>> {
    println!("Starting device scanning...");

    // Initially check if we have any neighbors to monitor
    let initial_count = neighbor_addresses.lock().await.len();
    if initial_count == 0 {
        println!("Warning: No neighbor addresses configured yet. Waiting for updates...");
    } else {
        println!("Monitoring {} device(s) initially", initial_count);
    }

    adapter
        .set_discovery_filter(DiscoveryFilter {
            // Only look for LE devices.
            transport: DiscoveryTransport::Le,

            // filter by service UUIDs.
            uuids: vec![BLUETOOTH_SERVICE_UUID.parse().unwrap()]
                .into_iter()
                .collect(),

            // Set discoverable to true
            discoverable: true,

            ..Default::default()
        })
        .await?;

    // Start discovery
    let discover = adapter.discover_devices().await?;
    tokio::pin!(discover);

    println!("Device scanning started...");

    // Track spawned tasks so we can abort them when devices are removed
    let mut device_tasks: HashMap<Address, task::JoinHandle<()>> = HashMap::new();

    // Continuously scan for devices
    loop {
        tokio::select! {
            Some(evt) = discover.next() => {
                match evt {
                    AdapterEvent::DeviceAdded(addr) => {
                        // Only process devices in our target list
                        let is_neighbor = neighbor_addresses.lock().await.contains(&addr);
                        if !is_neighbor {
                            continue;
                        }

                        // Skip if we already have a task for this device
                        if device_tasks.contains_key(&addr) {
                            continue;
                        }

                        let device = adapter.device(addr)?;

                        // Spawn a task to listen for RSSI changes on this device
                        let rssi_data_clone = Arc::clone(&rssi_data);

                        let rssi = device.rssi().await?.unwrap_or(0);
                        println!("Device added: {} (RSSI: {})", addr, rssi);

                        if rssi != 0 {
                            let mut data = rssi_data_clone.lock().await;
                            let deque = data.entry(addr).or_insert_with(VecDeque::new);

                            // Keep only the last MAX_RSSI_QUEUE_SIZE values
                            if deque.len() >= MAX_RSSI_QUEUE_SIZE {
                                deque.pop_front();
                            }
                            deque.push_back(rssi);
                        }

                        let task = tokio::spawn(async move {
                            if let Ok(events) = device.events().await {
                                tokio::pin!(events);

                                while let Some(event) = events.next().await {
                                    match event {
                                        DeviceEvent::PropertyChanged(DeviceProperty::Rssi(rssi)) => {
                                            // RSSI changed
                                            let mut data = rssi_data_clone.lock().await;
                                            let deque = data.entry(addr).or_insert_with(VecDeque::new);

                                            // Keep only the last MAX_RSSI_QUEUE_SIZE values
                                            if deque.len() >= MAX_RSSI_QUEUE_SIZE {
                                                deque.pop_front();
                                            }
                                            deque.push_back(rssi);

                                            println!("RSSI update for {}: {}", addr, rssi);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        });

                        device_tasks.insert(addr, task);
                    }
                    AdapterEvent::DeviceRemoved(addr) => {
                        // Clean up the task for this device
                        if let Some(task) = device_tasks.remove(&addr) {
                            task.abort();
                            println!("Device removed, task aborted: {}", addr);
                        }

                        // Also remove RSSI data
                        rssi_data.lock().await.remove(&addr);
                    }
                    _ => {}
                }
            }
            _ = time::sleep(Duration::from_millis(100)) => {
                // Just continue scanning
            }
        }
    }
}

pub async fn start_continuous_scan(
    adapter: Adapter,
    rssi_data: RssiData,
    neighbor_addresses: NeighborAddresses,
) -> Result<(), Box<dyn Error>> {
    println!("Starting continuous Bluetooth operations...");

    // Get the Bluetooth adapter
    println!(
        "Using adapter: {} ({})",
        adapter.address().await?,
        adapter.name()
    );

    // Power on the adapter if it's not already.
    adapter.set_powered(true).await?;

    // Make the adapter discoverable.
    adapter.set_discoverable(true).await?;

    // Set discoverable timeout to 0 (never timeout).
    adapter.set_discoverable_timeout(0).await?;

    // Clone adapter for the advertising task
    let adapter_clone = adapter.clone();

    // Spawn advertising task
    tokio::spawn(async move {
        if let Err(e) = start_advertising(&adapter_clone).await {
            eprintln!("Advertising error: {}", e);
        }
    });

    // Run device scanning (this blocks indefinitely)
    scan_devices(&adapter, rssi_data, neighbor_addresses).await
}

pub async fn current_rssi(rssi_data: RssiData) -> Result<RssiResponse, Box<dyn Error>> {
    println!("Calculating median RSSI from current data...");

    let rssi_data_snapshot = rssi_data.lock().await.clone();

    // Build response with median RSSI values
    let mut devices = Vec::new();
    for (address, rssi_deque) in rssi_data_snapshot {
        if !rssi_deque.is_empty() {
            let mut rssi_values: Vec<i16> = rssi_deque.into_iter().collect();
            if let Some(median_rssi) = calculate_median(&mut rssi_values) {
                devices.push(DeviceRssi {
                    address: address.0,
                    rssi: median_rssi,
                });
            }
        }
    }

    Ok(RssiResponse { devices })
}
