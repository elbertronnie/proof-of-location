use btleplug::api::{BDAddr, Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use futures::stream::StreamExt;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::Duration;
use tokio::time;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    println!("Starting Bluetooth RSSI Scanner...\n");

    // Get the list of target Bluetooth addresses to monitor
    let target_set = get_bluetooth_addresses();

    if target_set.is_empty() {
        eprintln!("No Bluetooth addresses configured in BLUETOOTH_ADDRESSES environment variable!");
        eprintln!("Please set BLUETOOTH_ADDRESSES with comma-separated addresses.");
        return Ok(());
    }

    println!("Monitoring {} device(s):", target_set.len());
    for addr in &target_set {
        println!("  - {}", addr);
    }
    println!();

    // Get the Bluetooth adapter
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;

    if adapters.is_empty() {
        eprintln!("No Bluetooth adapters found!");
        return Ok(());
    }

    let central = &adapters[0];
    println!("Using adapter: {:?}\n", central.adapter_info().await?);

    // Start scanning for devices
    central.start_scan(ScanFilter::default()).await?;
    println!("Scanning for 60 seconds...\n");

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

    println!("\n{}", "=".repeat(75));
    println!("Scan complete!\n");

    // Print median RSSI values
    println!(
        "{:<40} {:<20} {:>15} {:>10}",
        "Device Address", "Name", "Median RSSI", "Samples"
    );
    println!("{}", "=".repeat(90));

    for (address, rssi_values) in rssi_records.iter_mut() {
        let name = device_names
            .get(address)
            .map(|s| s.as_str())
            .unwrap_or("Unknown");
        let sample_count = rssi_values.len();

        if let Some(median) = calculate_median(rssi_values) {
            println!(
                "{:<40} {:<20} {:>13.1} dBm {:>10}",
                address, name, median, sample_count
            );
        }
    }

    println!("\n{}", "=".repeat(90));
    println!("Total devices found: {}", rssi_records.len());

    // Stop scanning
    central.stop_scan().await?;

    Ok(())
}
