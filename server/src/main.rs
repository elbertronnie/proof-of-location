use axum::{http::StatusCode, routing::get, Json, Router};
use btleplug::api::{BDAddr, Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::Duration;
use tokio::time;


#[derive(Serialize, Deserialize)]
struct DeviceRssi {
    address: [u8; 6],
    name: String,
    rssi: f64,
}

#[derive(Serialize, Deserialize)]
struct RssiResponse {
    devices: Vec<DeviceRssi>,
}

fn get_bluetooth_addresses() -> HashSet<BDAddr> {
    std::env::var("BLUETOOTH_ADDRESSES")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

fn calculate_median(values: &mut Vec<i16>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }

    values.sort_unstable();
    let len = values.len();

    if len % 2 == 0 {
        Some((values[len / 2 - 1] as f64 + values[len / 2] as f64) / 2.0)
    } else {
        Some(values[len / 2] as f64)
    }
}

async fn scan_rssi() -> Result<Json<RssiResponse>, (StatusCode, String)> {
    match perform_bluetooth_scan().await {
        Ok(response) => Ok(Json(response)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Scan failed: {}", e),
        )),
    }
}

async fn perform_bluetooth_scan() -> Result<RssiResponse, Box<dyn Error>> {
    println!("Starting Bluetooth RSSI scan...");

    // Get the list of target Bluetooth addresses to monitor
    let target_set = get_bluetooth_addresses();

    if target_set.is_empty() {
        return Err(
            "No Bluetooth addresses configured in BLUETOOTH_ADDRESSES environment variable".into(),
        );
    }

    println!("Monitoring {} device(s)", target_set.len());

    // Get the Bluetooth adapter
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;

    if adapters.is_empty() {
        return Err("No Bluetooth adapters found".into());
    }

    let central = &adapters[0];
    println!("Using adapter: {:?}", central.adapter_info().await?);

    // Start scanning for devices
    central.start_scan(ScanFilter::default()).await?;
    println!("Scanning for 60 seconds...");

    // Create a stream of events
    let mut events = central.events().await?;

    // Keep track of all RSSI values for each device
    let mut rssi_records: HashMap<BDAddr, Vec<i16>> = HashMap::new();
    let mut device_names: HashMap<BDAddr, String> = HashMap::new();

    // Scan for 60 seconds and record RSSI values
    let scan_duration = Duration::from_secs(60);
    let start_time = time::Instant::now();

    while start_time.elapsed() < scan_duration {
        // Check for new devices with timeout
        tokio::select! {
            Some(_event) = events.next() => {
                // When a device is discovered or updated, get all peripherals
                if let Ok(peripherals) = central.peripherals().await {
                    for peripheral in peripherals {
                        // Get peripheral properties
                        let props = peripheral.properties().await?;

                        if let Some(properties) = props {
                            // Only process devices in our target list
                            if !target_set.contains(&properties.address) {
                                continue;
                            }

                            let address = properties.address;
                            let name = properties
                                .local_name
                                .unwrap_or_else(|| "Unknown".to_string());
                            let rssi = properties.rssi.unwrap_or(0);

                            // Record RSSI value if non-zero
                            if rssi != 0 {
                                rssi_records.entry(address).or_insert_with(Vec::new).push(rssi);
                                device_names.insert(address, name);
                            }
                        }
                    }
                }
            }
            _ = time::sleep(Duration::from_millis(100)) => {
                // Continue scanning
            }
        }
    }

    println!("Scan complete!");

    // Stop scanning
    central.stop_scan().await?;

    // Build response with median RSSI values
    let mut devices = Vec::new();
    for (address, mut rssi_values) in rssi_records {
        if let Some(median_rssi) = calculate_median(&mut rssi_values) {
            let name = device_names.get(&address).cloned().unwrap_or_else(|| "Unknown".to_string());
            devices.push(DeviceRssi {
                address: address.into_inner(),
                name,
                rssi: median_rssi,
            });
        }
    }

    Ok(RssiResponse { devices })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    println!("Starting Bluetooth RSSI Scanner Server...\n");

    // Build the Axum router
    let app = Router::new().route("/rssi", get(scan_rssi));

    // Get the server port from environment or use default
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Server listening on http://{}", addr);
    println!("Access the RSSI endpoint at: http://{}/rssi\n", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
