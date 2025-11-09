mod bluetooth;
mod neighbor;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use bluer::{Adapter, Session};
use codec::{Decode, Encode};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use bluetooth::{
    bluetooth_address, current_rssi, init_neighbor_addresses_from_env, start_continuous_scan,
    NeighborAddresses, RssiData,
};
use neighbor::{calculate_neighbors, fetch_max_distance, start_neighbor_event_listener};
use subxt::{OnlineClient, SubstrateConfig};

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

#[derive(Clone)]
struct AppState {
    adapter: Adapter,
    rssi_data: RssiData,
}

async fn scan_rssi(State(state): State<AppState>, req: Request) -> impl IntoResponse {
    // Extract and log the Node ID from the X-Node-ID header
    let node_id = req
        .headers()
        .get("X-Node-ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    println!("📡 RSSI request from node: {}", node_id);

    match current_rssi(state.rssi_data).await {
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

async fn get_location(State(state): State<AppState>, req: Request) -> impl IntoResponse {
    // Extract and log the Node ID from the X-Node-ID header
    let node_id = req
        .headers()
        .get("X-Node-ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    println!("📍 Location request from node: {}", node_id);

    // Get latitude and longitude from environment variables
    let latitude = std::env::var("LATITUDE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let longitude = std::env::var("LONGITUDE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let address = bluetooth_address(&state.adapter).await;

    let response = LocationResponse {
        address: address.0,
        location: Location {
            latitude,
            longitude,
        },
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
    dotenvy::dotenv()?;

    println!("Starting Bluetooth RSSI Scanner Server...\n");

    // Create Bluetooth session
    let session = Session::new()
        .await
        .expect("Failed to create Bluetooth session");
    let adapter = session
        .default_adapter()
        .await
        .expect("Failed to get default adapter");

    // Get our Bluetooth address
    let our_bluetooth_address = bluetooth_address(&adapter).await;
    println!("Our Bluetooth address: {}", our_bluetooth_address);

    // Connect to the Substrate node
    let substrate_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string());
    println!("Connecting to Substrate node at: {}", substrate_url);

    let api = OnlineClient::<SubstrateConfig>::from_url(&substrate_url)
        .await
        .expect("Failed to connect to Substrate node");
    println!("Connected to Substrate node successfully\n");

    // Get max distance
    let max_distance_meters = fetch_max_distance(&api);
    println!(
        "Max distance for neighbors: {} meters\n",
        max_distance_meters
    );

    // Create shared state for RSSI data
    let rssi_data: RssiData = Arc::new(Mutex::new(HashMap::new()));

    // Create shared state for neighbor addresses
    // Initialize with env variable if available (for backwards compatibility)
    let initial_neighbors = init_neighbor_addresses_from_env();
    let neighbor_addresses: NeighborAddresses = Arc::new(Mutex::new(initial_neighbors));

    // Calculate neighbors once at startup
    println!("Calculating initial neighbor list...");
    match calculate_neighbors(&api, our_bluetooth_address, max_distance_meters).await {
        Ok(neighbors) => {
            let mut addr_lock = neighbor_addresses.lock().await;
            *addr_lock = neighbors;
            println!("✅ Initial neighbor count: {}", addr_lock.len());
        }
        Err(e) => {
            eprintln!("⚠️  Failed to calculate initial neighbors: {}", e);
        }
    }

    // Start listening for NodeRegistered events and auto-update neighbor list
    start_neighbor_event_listener(
        api.clone(),
        our_bluetooth_address,
        max_distance_meters,
        Arc::clone(&neighbor_addresses),
    )
    .await;

    // Spawn background task for continuous Bluetooth scanning
    let adapter_clone = adapter.clone();
    let rssi_data_clone = Arc::clone(&rssi_data);
    let neighbor_addresses_clone = Arc::clone(&neighbor_addresses);
    tokio::spawn(async move {
        if let Err(e) =
            start_continuous_scan(adapter_clone, rssi_data_clone, neighbor_addresses_clone).await
        {
            eprintln!("Bluetooth scan error: {}", e);
        }
    });

    // Create app state
    let app_state = AppState { adapter, rssi_data };

    // Build the Axum router
    let app = Router::new()
        .route("/rssi", get(scan_rssi))
        .route("/location", get(get_location))
        .with_state(app_state);

    // Get the server port from environment or use default
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("Server listening on http://{}", addr);
    println!("Access the RSSI endpoint at: http://{}/rssi", addr);
    println!(
        "Access the Location endpoint at: http://{}/location\n",
        addr
    );

    // Start the server
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
