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
    println!("Scanning for Bluetooth devices...\n");
    println!(
        "{:<40} {:<20} {:>10}",
        "Device Address", "Name", "RSSI (dBm)"
    );
    println!("{}", "=".repeat(75));

    // Create a stream of events
    let mut events = central.events().await?;

    // Keep track of devices we've already printed
    let mut seen_devices: HashMap<String, i16> = HashMap::new();

    // Scan for 60 seconds and print RSSI values
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

                            let address = properties.address.to_string();
                            let name = properties
                                .local_name
                                .unwrap_or_else(|| "Unknown".to_string());
                            let rssi = properties.rssi.unwrap_or(0);

                            // Only print if RSSI has changed or is new
                            if rssi != 0 {
                                let should_print = seen_devices
                                    .get(&address)
                                    .map_or(true, |&old_rssi| old_rssi != rssi);

                                if should_print {
                                    println!("{:<40} {:<20} {:>10} dBm", address, name, rssi);
                                    seen_devices.insert(address, rssi);
                                }
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
    println!("Scan complete!");
    println!("Total devices found: {}", seen_devices.len());

    // Stop scanning
    central.stop_scan().await?;

    Ok(())
}
