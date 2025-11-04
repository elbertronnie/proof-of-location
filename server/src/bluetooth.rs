use bluer::{AdapterEvent, Address, DeviceEvent, DeviceProperty};
use codec::{Decode, Encode};
use futures::stream::StreamExt;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;

const MAX_RSSI_QUEUE_SIZE: usize = 5;

#[derive(Encode, Decode, Debug, Clone)]
pub struct DeviceRssi {
    pub address: [u8; 6],
    pub name: Vec<u8>,
    pub rssi: i16,
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct RssiResponse {
    pub devices: Vec<DeviceRssi>,
}

pub fn get_neighbour_addresses() -> HashSet<Address> {
    std::env::var("BLUETOOTH_ADDRESSES")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

pub fn get_bluetooth_address() -> Address {
    let addr_str =
        std::env::var("BLUETOOTH_ADDRESS").expect("BLUETOOTH_ADDRESS environment variable not set");
    addr_str.parse().expect("Invalid BLUETOOTH_ADDRESS format")
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
pub type DeviceNames = Arc<Mutex<HashMap<Address, String>>>;

pub async fn start_continuous_scan(
    rssi_data: RssiData,
    device_names: DeviceNames,
) -> Result<(), Box<dyn Error>> {
    println!("Starting continuous Bluetooth RSSI scan...");

    // Get the list of target Bluetooth addresses to monitor
    let target_set = get_neighbour_addresses();

    if target_set.is_empty() {
        return Err(
            "No Bluetooth addresses configured in BLUETOOTH_ADDRESSES environment variable".into(),
        );
    }

    println!("Monitoring {} device(s)", target_set.len());

    // Get the Bluetooth adapter
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    println!("Using adapter: {}", adapter.name());

    // Power on the adapter if it's not already.
    adapter.set_powered(true).await?;

    // Make the adapter discoverable.
    adapter.set_discoverable(true).await?;

    // Start discovery
    let discover = adapter.discover_devices().await?;
    tokio::pin!(discover);

    println!("Continuous scanning started...");

    // Track spawned tasks so we can abort them when devices are removed
    let mut device_tasks: HashMap<Address, tokio::task::JoinHandle<()>> = HashMap::new();

    // Continuously scan for devices
    loop {
        tokio::select! {
            Some(evt) = discover.next() => {
                match evt {
                    AdapterEvent::DeviceAdded(addr) => {
                        // Only process devices in our target list
                        if !target_set.contains(&addr) {
                            continue;
                        }

                        // Skip if we already have a task for this device
                        if device_tasks.contains_key(&addr) {
                            continue;
                        }

                        println!("Device added: {}", addr);

                        let device = adapter.device(addr)?;

                        // Get initial device name
                        if let Ok(Some(name)) = device.name().await {
                            device_names.lock().await.insert(addr, name);
                        }

                        // Spawn a task to listen for RSSI changes on this device
                        let rssi_data_clone = Arc::clone(&rssi_data);
                        let device_names_clone = Arc::clone(&device_names);

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
                                        DeviceEvent::PropertyChanged(DeviceProperty::Name(name)) => {
                                            // Name changed/became available
                                            println!("Name update for {}: {}", addr, name);
                                            device_names_clone.lock().await.insert(addr, name);
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

pub async fn get_current_rssi(
    rssi_data: RssiData,
    device_names: DeviceNames,
) -> Result<RssiResponse, Box<dyn Error>> {
    println!("Calculating median RSSI from current data...");

    let rssi_data_snapshot = rssi_data.lock().await.clone();
    let device_names_snapshot = device_names.lock().await.clone();

    // Build response with median RSSI values
    let mut devices = Vec::new();
    for (address, rssi_deque) in rssi_data_snapshot {
        if !rssi_deque.is_empty() {
            let mut rssi_values: Vec<i16> = rssi_deque.into_iter().collect();
            if let Some(median_rssi) = calculate_median(&mut rssi_values) {
                let name = device_names_snapshot
                    .get(&address)
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string());
                devices.push(DeviceRssi {
                    address: address.0,
                    name: name.into_bytes(),
                    rssi: median_rssi,
                });
            }
        }
    }

    Ok(RssiResponse { devices })
}
