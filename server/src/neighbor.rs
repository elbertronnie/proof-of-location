use bluer::Address;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use subxt::{OnlineClient, SubstrateConfig};
use tokio::sync::Mutex;

use substrate::proof_of_location::events::{NodeRegistered, NodeUnregistered, NodeUpdated};
use substrate::runtime_types::pallet_proof_of_location::util::LocationData;

// This creates a complete, type-safe API for interacting with the runtime.
#[subxt::subxt(runtime_metadata_path = "../metadata.scale")]
pub mod substrate {}

/// Cached location coordinates (latitude, longitude)
/// Read once from environment variables and reused throughout the application
static CACHED_LOCATION: OnceLock<(f64, f64)> = OnceLock::new();

/// Get our location from cache or initialize from environment variables
pub fn get_our_location() -> (f64, f64) {
    *CACHED_LOCATION.get_or_init(|| {
        let lat = std::env::var("LATITUDE")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let lon = std::env::var("LONGITUDE")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        (lat, lon)
    })
}

/// Fetch all location data from the chain
pub async fn fetch_all_location_data(
    api: &OnlineClient<SubstrateConfig>,
) -> Result<HashMap<[u8; 32], LocationData>, String> {
    // Use the generated API to access the storage
    let query = substrate::storage().proof_of_location().account_data_iter();

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

/// Fetch the MaxDistance constant from the runtime.
///
/// Falls back to default of 10 meters.
pub fn fetch_max_distance(api: &OnlineClient<SubstrateConfig>) -> u32 {
    let query = substrate::constants().proof_of_location().max_distance();
    api.constants().at(&query).unwrap_or(10) // Default value matching the runtime constant
}

/// Calculate which nodes are neighbors based on distance from our location
///
/// A neighbor is defined as a node whose distance from us is less than max_distance
pub async fn calculate_neighbors(
    api: &OnlineClient<SubstrateConfig>,
    our_bluetooth_address: Address,
    max_distance: u32,
) -> Result<HashSet<Address>, String> {
    let all_location_data = fetch_all_location_data(api).await?;

    // Get our cached location
    let (our_lat, our_lon) = get_our_location();

    // Find all neighbors within max_distance
    let mut neighbors = HashSet::new();

    for location_data in all_location_data.values() {
        // Skip ourselves
        if location_data.address == our_bluetooth_address.0 {
            continue;
        }

        let their_lat = location_data.latitude as f64 / 1_000_000.0;
        let their_lon = location_data.longitude as f64 / 1_000_000.0;

        let dist = distance(our_lat, our_lon, their_lat, their_lon);

        if dist <= max_distance as f64 {
            // Convert [u8; 6] to Address
            neighbors.insert(Address(location_data.address));
        }
    }

    Ok(neighbors)
}

/// Calculate distance between two coordinates in meters
fn distance(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> f64 {
    use haversine_redux::Location;
    let a = Location::new(a_lat, a_lon);
    let b = Location::new(b_lat, b_lon);
    a.kilometers_to(&b) * 1000.0 // convert kilometers to meters
}

/// Helper function to calculate distance from our cached location to a given coordinate
fn calculate_distance_from_us(latitude: i64, longitude: i64) -> f64 {
    let (our_lat, our_lon) = get_our_location();
    let their_lat = latitude as f64 / 1_000_000.0;
    let their_lon = longitude as f64 / 1_000_000.0;
    distance(our_lat, our_lon, their_lat, their_lon)
}

/// Handle adding a node as a neighbor if it's within range
async fn handle_node_in_range(
    address: [u8; 6],
    latitude: i64,
    longitude: i64,
    neighbor_addresses: &Arc<Mutex<HashSet<Address>>>,
    max_distance: u32,
    event_type: &str,
) {
    let dist = calculate_distance_from_us(latitude, longitude);
    let node_address = Address(address);

    if dist <= max_distance as f64 {
        let mut addr_lock = neighbor_addresses.lock().await;
        if addr_lock.insert(node_address) {
            println!(
                "✅ {} neighbor: {} (distance: {:.2}m) - Total neighbors: {}",
                event_type,
                node_address,
                dist,
                addr_lock.len()
            );
        } else if event_type == "Updated" {
            println!(
                "🔄 Updated neighbor location: {} (distance: {:.2}m)",
                node_address, dist
            );
        }
    } else {
        println!(
            "⏭️  Node {:?} is too far away ({:.2}m > {}m), not adding as neighbor",
            address, dist, max_distance
        );
    }
}

/// Handle removing a node from neighbors if it's out of range
async fn handle_node_out_of_range(
    address: [u8; 6],
    latitude: i64,
    longitude: i64,
    neighbor_addresses: &Arc<Mutex<HashSet<Address>>>,
    max_distance: u32,
) {
    let dist = calculate_distance_from_us(latitude, longitude);
    let node_address = Address(address);

    if dist > max_distance as f64 {
        let mut addr_lock = neighbor_addresses.lock().await;
        if addr_lock.remove(&node_address) {
            println!(
                "❌ Removed neighbor (moved too far): {} (distance: {:.2}m > {}m) - Total neighbors: {}",
                node_address, dist, max_distance, addr_lock.len()
            );
        } else {
            println!(
                "⏭️  Updated node is not a neighbor ({:.2}m > {}m)",
                dist, max_distance
            );
        }
    }
}

/// Start listening to NodeRegistered events and update the neighbor list automatically
/// This function spawns a background task that subscribes to blockchain events
pub async fn start_neighbor_event_listener(
    api: OnlineClient<SubstrateConfig>,
    our_bluetooth_address: Address,
    max_distance: u32,
    neighbor_addresses: Arc<Mutex<HashSet<Address>>>,
) {
    tokio::spawn(async move {
        println!("🎧 Starting node event listener...\n");

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

                        // Find and process node events using subxt generated API
                        for event_result in events.iter() {
                            let event = match event_result {
                                Ok(event) => event,
                                Err(e) => {
                                    eprintln!("⚠️  Failed to get event: {}", e);
                                    continue;
                                }
                            };

                            // Handle NodeRegistered event
                            if let Ok(Some(node_registered)) = event.as_event::<NodeRegistered>() {
                                if node_registered.address == our_bluetooth_address.0 {
                                    continue;
                                }

                                println!(
                                    "📍 NodeRegistered event detected for address: {:?}",
                                    node_registered.address
                                );

                                handle_node_in_range(
                                    node_registered.address,
                                    node_registered.latitude,
                                    node_registered.longitude,
                                    &neighbor_addresses,
                                    max_distance,
                                    "Added new",
                                )
                                .await;
                            }

                            // Handle NodeUnregistered event
                            if let Ok(Some(node_unregistered)) =
                                event.as_event::<NodeUnregistered>()
                            {
                                let removed_address = Address(node_unregistered.address);

                                if removed_address == our_bluetooth_address {
                                    continue;
                                }

                                println!(
                                    "🗑️  NodeUnregistered event detected for address: {:?}",
                                    node_unregistered.address
                                );

                                let mut addr_lock = neighbor_addresses.lock().await;
                                if addr_lock.remove(&removed_address) {
                                    println!(
                                        "❌ Removed neighbor: {} - Total neighbors: {}",
                                        removed_address,
                                        addr_lock.len()
                                    );
                                } else {
                                    println!(
                                        "⏭️  Node {:?} was not in neighbor list",
                                        node_unregistered.address
                                    );
                                }
                            }

                            // Handle NodeUpdated event
                            if let Ok(Some(node_updated)) = event.as_event::<NodeUpdated>() {
                                let old_address = Address(node_updated.old_address);
                                let new_address = Address(node_updated.new_address);

                                if new_address == our_bluetooth_address {
                                    continue;
                                }

                                println!(
                                    "🔄 NodeUpdated event detected - Old: {:?}, New: {:?}",
                                    node_updated.old_address, node_updated.new_address
                                );

                                // Remove old address if it changed
                                if old_address != new_address {
                                    let mut addr_lock = neighbor_addresses.lock().await;
                                    addr_lock.remove(&old_address);
                                }

                                // Calculate distance and determine if node should be a neighbor
                                let dist = calculate_distance_from_us(
                                    node_updated.new_latitude,
                                    node_updated.new_longitude,
                                );

                                if dist <= max_distance as f64 {
                                    handle_node_in_range(
                                        node_updated.new_address,
                                        node_updated.new_latitude,
                                        node_updated.new_longitude,
                                        &neighbor_addresses,
                                        max_distance,
                                        "Updated",
                                    )
                                    .await;
                                } else {
                                    handle_node_out_of_range(
                                        node_updated.new_address,
                                        node_updated.new_latitude,
                                        node_updated.new_longitude,
                                        &neighbor_addresses,
                                        max_distance,
                                    )
                                    .await;
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
