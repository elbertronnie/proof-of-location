use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use btleplug::api::{BDAddr, Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use codec::{Decode, Encode};
use futures::stream::StreamExt;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::Duration;
use tokio::time;

#[derive(Encode, Decode, Debug, Clone)]
struct DeviceRssi {
    address: [u8; 6],
    name: Vec<u8>,
    rssi: i16,
}

#[derive(Encode, Decode, Debug, Clone)]
struct RssiResponse {
    devices: Vec<DeviceRssi>,
}

#[derive(Encode, Decode, Debug, Clone)]
struct Location {
    latitude: f64,
    longitude: f64,
}

#[derive(Encode, Decode, Debug, Clone)]
struct LocationResponse {
    address: [u8; 6],
    location: Location,
}

fn get_neighbour_addresses() -> HashSet<BDAddr> {
    std::env::var("BLUETOOTH_ADDRESSES")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

fn get_bluetooth_address() -> BDAddr {
    let addr_str = std::env::var("BLUETOOTH_ADDRESS")
        .expect("BLUETOOTH_ADDRESS environment variable not set");
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

async fn scan_rssi() -> impl IntoResponse {
    match perform_bluetooth_scan().await {
        Ok(response) => {
            // Encode the response using SCALE codec
            let encoded = response.encode();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(encoded))
                .unwrap()
        }
        Err(e) => {
            let error_msg = format!("Scan failed: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(error_msg))
                .unwrap()
        }
    }
}

async fn perform_bluetooth_scan() -> Result<RssiResponse, Box<dyn Error>> {
    println!("Starting Bluetooth RSSI scan...");

    // Get the list of target Bluetooth addresses to monitor
    let target_set = get_neighbour_addresses();

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
            let name = device_names
                .get(&address)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            devices.push(DeviceRssi {
                address: address.into_inner(),
                name: name.into_bytes(),
                rssi: median_rssi,
            });
        }
    }

    Ok(RssiResponse { devices })
}

async fn get_location() -> impl IntoResponse {
    // Get latitude and longitude from environment variables
    let latitude = std::env::var("LATITUDE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    
    let longitude = std::env::var("LONGITUDE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let address = get_bluetooth_address();

    let response = LocationResponse {
        address: address.into_inner(),
        location: Location { latitude, longitude },
    };

    // Encode the response using SCALE codec
    let encoded = response.encode();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(encoded))
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    println!("Starting Bluetooth RSSI Scanner Server...\n");

    // Build the Axum router
    let app = Router::new()
        .route("/rssi", get(scan_rssi))
        .route("/location", get(get_location));

    // Get the server port from environment or use default
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Server listening on http://{}", addr);
    println!("Access the RSSI endpoint at: http://{}/rssi", addr);
    println!("Access the Location endpoint at: http://{}/location\n", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
