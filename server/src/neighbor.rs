use bluer::Address;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use subxt::{OnlineClient, SubstrateConfig};
use tokio::sync::Mutex;

use substrate::runtime_types::pallet_template::pallet::LocationData;

// This creates a complete, type-safe API for interacting with the runtime.
#[subxt::subxt(runtime_metadata_path = "../metadata.scale")]
pub mod substrate {}

/// Calculate distance between two coordinates in meters
pub fn distance(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> f64 {
    use haversine_redux::Location;
    let a = Location::new(a_lat, a_lon);
    let b = Location::new(b_lat, b_lon);
    a.kilometers_to(&b) * 1000.0 // convert kilometers to meters
}

/// Fetch all location data from the chain
pub async fn fetch_all_location_data(
    api: &OnlineClient<SubstrateConfig>,
) -> Result<HashMap<[u8; 32], LocationData>, String> {
    // Use the generated API to access the storage
    let query = substrate::storage().template().account_data_iter();

    // Fetch all account data
    let mut account_data = api
        .storage()
        .at_latest()
        .await
        .map_err(|e| e.to_string())?
        .iter(query)
        .await
        .map_err(|e| e.to_string())?;

    // Store results inside a HashMap
    let mut results = HashMap::new();

    // Collect all results
    while let Some(Ok(data)) = account_data.next().await {
        // Skip first 32 bytes (pallet + storage hashes) and 16 bytes (blake2_128 hash)
        // Take the last 32 bytes as the account ID
        let len = data.key_bytes.len();
        let account_id: [u8; 32] = data.key_bytes[len - 32..]
            .try_into()
            .map_err(|e| format!("Failed to convert key bytes to account ID: {:?}", e))?;
        results.insert(account_id, data.value);
    }

    Ok(results)
}

/// Fetch the MaxDistanceMeters constant from the runtime
/// Falls back to default of 10 meters
pub fn fetch_max_distance(api: &OnlineClient<SubstrateConfig>) -> u32 {
    let query = substrate::constants().template().max_distance_meters();
    api.constants().at(&query).unwrap_or(10) // Default value matching the runtime constant
}

/// Calculate which nodes are neighbors based on distance from our location
/// A neighbor is defined as a node whose distance from us is less than max_distance_meters
pub async fn calculate_neighbors(
    api: &OnlineClient<SubstrateConfig>,
    our_bluetooth_address: Address,
    max_distance_meters: u32,
) -> Result<HashSet<Address>, String> {
    let all_location_data = fetch_all_location_data(api).await?;

    // Find our own location data by env variables LATITUDE and LONGITUDE
    let our_lat = std::env::var("LATITUDE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let our_lon = std::env::var("LONGITUDE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let our_lat = our_lat / 1_000_000.0;
    let our_lon = our_lon / 1_000_000.0;

    // Find all neighbors within max_distance_meters
    let mut neighbors = HashSet::new();

    for location_data in all_location_data.values() {
        // Skip ourselves
        if location_data.address == our_bluetooth_address.0 {
            continue;
        }

        let their_lat = location_data.latitude as f64 / 1_000_000.0;
        let their_lon = location_data.longitude as f64 / 1_000_000.0;

        let dist = distance(our_lat, our_lon, their_lat, their_lon);

        if dist <= max_distance_meters as f64 {
            // Convert [u8; 6] to Address
            neighbors.insert(Address(location_data.address));
        }
    }

    Ok(neighbors)
}

/// Start listening to NodeRegistered events and update the neighbor list automatically
/// This function spawns a background task that subscribes to blockchain events
pub async fn start_neighbor_event_listener(
    api: OnlineClient<SubstrateConfig>,
    our_bluetooth_address: Address,
    max_distance_meters: u32,
    neighbor_addresses: Arc<Mutex<HashSet<Address>>>,
) {
    tokio::spawn(async move {
        println!("🎧 Starting NodeRegistered event listener...\n");

        loop {
            // Subscribe to finalized blocks
            let mut blocks_sub = match api.blocks().subscribe_finalized().await {
                Ok(sub) => sub,
                Err(e) => {
                    eprintln!("⚠️  Failed to subscribe to blocks: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            // Process each finalized block
            while let Some(block_result) = blocks_sub.next().await {
                match block_result {
                    Ok(block) => {
                        // Get events from this block
                        let events = match block.events().await {
                            Ok(events) => events,
                            Err(e) => {
                                eprintln!("⚠️  Failed to fetch events: {}", e);
                                continue;
                            }
                        };

                        // Find and process NodeRegistered events using subxt generated API
                        for event_result in events.iter() {
                            let event = match event_result {
                                Ok(event) => event,
                                Err(e) => {
                                    eprintln!("⚠️  Failed to get event: {}", e);
                                    continue;
                                }
                            };

                            // Try to decode as NodeRegistered event
                            if let Ok(Some(node_registered)) =
                                event.as_event::<substrate::template::events::NodeRegistered>()
                            {
                                // Skip if it's ourselves
                                if node_registered.address == our_bluetooth_address.0 {
                                    continue;
                                }

                                println!(
                                    "📍 NodeRegistered event detected for address: {:?}",
                                    node_registered.address
                                );

                                // Get our location from environment
                                let our_lat = std::env::var("LATITUDE")
                                    .ok()
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0)
                                    / 1_000_000.0;
                                let our_lon = std::env::var("LONGITUDE")
                                    .ok()
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(0.0)
                                    / 1_000_000.0;

                                // Convert the new node's location
                                let their_lat = node_registered.latitude as f64 / 1_000_000.0;
                                let their_lon = node_registered.longitude as f64 / 1_000_000.0;

                                // Calculate distance to the new node
                                let dist = distance(our_lat, our_lon, their_lat, their_lon);

                                // Check if this new node is a neighbor
                                if dist <= max_distance_meters as f64 {
                                    let new_neighbor_address = Address(node_registered.address);
                                    let mut addr_lock = neighbor_addresses.lock().await;

                                    // Add to neighbors if not already present
                                    if addr_lock.insert(new_neighbor_address) {
                                        println!("✅ Added new neighbor: {} (distance: {:.2}m) - Total neighbors: {}", 
                                            new_neighbor_address, dist, addr_lock.len());
                                    }
                                } else {
                                    println!("⏭️  Node {:?} is too far away ({:.2}m > {}m), not adding as neighbor",
                                        node_registered.address, dist, max_distance_meters);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  Error processing block: {}", e);
                    }
                }
            }

            // If subscription ends, wait a bit and reconnect
            eprintln!("⚠️  Block subscription ended, reconnecting in 5s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });
}
