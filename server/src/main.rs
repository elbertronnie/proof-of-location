mod bluetooth;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use codec::{Decode, Encode};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    rssi_data: bluetooth::RssiData,
    device_names: bluetooth::DeviceNames,
}

async fn scan_rssi(State(state): State<AppState>, req: Request) -> impl IntoResponse {
    // Extract and log the Node ID from the X-Node-ID header
    let node_id = req
        .headers()
        .get("X-Node-ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    println!("📡 RSSI request from node: {}", node_id);

    match bluetooth::get_current_rssi(state.rssi_data, state.device_names).await {
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

async fn get_location(req: Request) -> impl IntoResponse {
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

    let address = bluetooth::get_bluetooth_address();

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
    dotenvy::dotenv().ok();

    println!("Starting Bluetooth RSSI Scanner Server...\n");

    // Create shared state for RSSI data
    let rssi_data: bluetooth::RssiData = Arc::new(Mutex::new(HashMap::new()));
    let device_names: bluetooth::DeviceNames = Arc::new(Mutex::new(HashMap::new()));

    // Spawn background task for continuous Bluetooth scanning
    let rssi_data_clone = Arc::clone(&rssi_data);
    let device_names_clone = Arc::clone(&device_names);
    tokio::spawn(async move {
        if let Err(e) = bluetooth::start_continuous_scan(rssi_data_clone, device_names_clone).await
        {
            eprintln!("Bluetooth scan error: {}", e);
        }
    });

    // Create app state
    let app_state = AppState {
        rssi_data,
        device_names,
    };

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
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
