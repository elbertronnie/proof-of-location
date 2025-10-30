use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use codec::{Decode, Encode};
use std::error::Error;

const ALICE_NODE_ID: &str = "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d";
const ALICE_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:01";
const ALICE_LATITUDE: f64 = 0.0;
const ALICE_LONGITUDE: f64 = 0.0;

const BOB_NODE_ID: &str = "0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48";
const BOB_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:02";
const BOB_LATITUDE: f64 = 0.0001;
const BOB_LONGITUDE: f64 = 0.0;

const CHARLIE_NODE_ID: &str = "0x90b5bbe3dba8afc0d6b2f5502b6f3c4f1edb3e4f6f9b1f3c4e5d6a7b8c9d0e1f";
const CHARLIE_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:03";
const CHARLIE_LATITUDE: f64 = -0.0001;
const CHARLIE_LONGITUDE: f64 = 0.0;

const DAVE_NODE_ID: &str = "0x2a1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef0123456789abcdef";
const DAVE_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:04";
const DAVE_LATITUDE: f64 = 0.0;
const DAVE_LONGITUDE: f64 = 0.0001;

const EVE_NODE_ID: &str = "0xfedcba98765432100123456789abcdef0123456789abcdef0123456789abcd";
const EVE_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:05";
const EVE_LATITUDE: f64 = 0.0;
const EVE_LONGITUDE: f64 = -0.0001;

const PATH_LOSS_EXPONENT: f64 = 3.0;

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

fn estimate_rssi(a_latitude: f64, a_longitude: f64, b_latitude: f64, b_longitude: f64) -> i16 {
    use haversine_rs::{distance, point::Point, units::Unit};

    let a = Point::new(a_latitude, a_longitude);
    let b = Point::new(b_latitude, b_longitude);
    let distance = distance(a, b, Unit::Meters);

    let rssi = -30.0 - PATH_LOSS_EXPONENT * 10.0 * distance.log10();
    rssi as i16
}

fn parse_bluetooth_address(addr_str: &str) -> Result<[u8; 6], Box<dyn Error>> {
    let parts: Vec<&str> = addr_str.split(':').collect();
    if parts.len() != 6 {
        return Err("Invalid Bluetooth address format".into());
    }

    let mut address = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        address[i] = u8::from_str_radix(part, 16)?;
    }

    Ok(address)
}

fn get_node_info(node_id: &str) -> Option<(&str, &str, f64, f64)> {
    match node_id {
        ALICE_NODE_ID => Some((
            "Alice",
            ALICE_BLUETOOTH_ADDRESS,
            ALICE_LATITUDE,
            ALICE_LONGITUDE,
        )),
        BOB_NODE_ID => Some((
            "Bob",
            BOB_BLUETOOTH_ADDRESS,
            BOB_LATITUDE,
            BOB_LONGITUDE,
        )),
        CHARLIE_NODE_ID => Some((
            "Charlie",
            CHARLIE_BLUETOOTH_ADDRESS,
            CHARLIE_LATITUDE,
            CHARLIE_LONGITUDE,
        )),
        DAVE_NODE_ID => Some((
            "Dave",
            DAVE_BLUETOOTH_ADDRESS,
            DAVE_LATITUDE,
            DAVE_LONGITUDE,
        )),
        EVE_NODE_ID => Some((
            "Eve",
            EVE_BLUETOOTH_ADDRESS,
            EVE_LATITUDE,
            EVE_LONGITUDE,
        )),
        _ => None,
    }
}

fn get_all_nodes() -> Vec<(&'static str, &'static str, &'static str, f64, f64)> {
    vec![
        (
            ALICE_NODE_ID,
            "Alice",
            ALICE_BLUETOOTH_ADDRESS,
            ALICE_LATITUDE,
            ALICE_LONGITUDE,
        ),
        (
            BOB_NODE_ID,
            "Bob",
            BOB_BLUETOOTH_ADDRESS,
            BOB_LATITUDE,
            BOB_LONGITUDE,
        ),
        (
            CHARLIE_NODE_ID,
            "Charlie",
            CHARLIE_BLUETOOTH_ADDRESS,
            CHARLIE_LATITUDE,
            CHARLIE_LONGITUDE,
        ),
        (
            DAVE_NODE_ID,
            "Dave",
            DAVE_BLUETOOTH_ADDRESS,
            DAVE_LATITUDE,
            DAVE_LONGITUDE,
        ),
        (
            EVE_NODE_ID,
            "Eve",
            EVE_BLUETOOTH_ADDRESS,
            EVE_LATITUDE,
            EVE_LONGITUDE,
        ),
    ]
}

async fn scan_rssi(req: Request) -> impl IntoResponse {
    // Extract the Node ID from the X-Node-ID header
    let node_id = req
        .headers()
        .get("X-Node-ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    println!("📡 RSSI request from node: {}", node_id);

    // Get the requesting node's location
    let (_, _, requester_lat, requester_lon) = match get_node_info(node_id) {
        Some(info) => info,
        None => {
            let error_msg = format!("Unknown node ID: {}", node_id);
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(error_msg))
                .unwrap();
        }
    };

    // Get all other nodes and calculate RSSI to each
    let mut devices = Vec::new();
    for (other_node_id, name, bluetooth_addr_str, other_lat, other_lon) in get_all_nodes() {
        // Skip the requesting node itself
        if other_node_id == node_id {
            continue;
        }

        // Parse the Bluetooth address
        let address = match parse_bluetooth_address(bluetooth_addr_str) {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("Failed to parse Bluetooth address for {}: {}", name, e);
                continue;
            }
        };

        // Calculate RSSI based on distance
        let rssi = estimate_rssi(requester_lat, requester_lon, other_lat, other_lon);

        devices.push(DeviceRssi {
            address,
            name: name.as_bytes().to_vec(),
            rssi,
        });

        println!(
            "  {} ({}): RSSI = {} dBm",
            name, bluetooth_addr_str, rssi
        );
    }

    println!("Returning RSSI data for {} devices\n", devices.len());

    let response = RssiResponse { devices };

    // Encode the response using SCALE codec
    let encoded = response.encode();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(encoded))
        .unwrap()
}

async fn get_location(req: Request) -> impl IntoResponse {
    // Extract the Node ID from the X-Node-ID header
    let node_id = req
        .headers()
        .get("X-Node-ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    println!("📍 Location request from node: {}", node_id);

    // Match the node ID to return the appropriate location
    let (bluetooth_address_str, latitude, longitude) = match node_id {
        ALICE_NODE_ID => (ALICE_BLUETOOTH_ADDRESS, ALICE_LATITUDE, ALICE_LONGITUDE),
        BOB_NODE_ID => (BOB_BLUETOOTH_ADDRESS, BOB_LATITUDE, BOB_LONGITUDE),
        CHARLIE_NODE_ID => (
            CHARLIE_BLUETOOTH_ADDRESS,
            CHARLIE_LATITUDE,
            CHARLIE_LONGITUDE,
        ),
        DAVE_NODE_ID => (DAVE_BLUETOOTH_ADDRESS, DAVE_LATITUDE, DAVE_LONGITUDE),
        EVE_NODE_ID => (EVE_BLUETOOTH_ADDRESS, EVE_LATITUDE, EVE_LONGITUDE),
        _ => {
            let error_msg = format!("Unknown node ID: {}", node_id);
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(error_msg))
                .unwrap();
        }
    };

    // Parse the Bluetooth address
    let address = match parse_bluetooth_address(bluetooth_address_str) {
        Ok(addr) => addr,
        Err(e) => {
            let error_msg = format!("Failed to parse Bluetooth address: {}", e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(error_msg))
                .unwrap();
        }
    };

    let response = LocationResponse {
        address,
        location: Location {
            latitude,
            longitude,
        },
    };

    println!(
        "Returning location for node {}: lat={}, lon={}",
        node_id, latitude, longitude
    );

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
    println!("Starting Location Simulator Server...\n");

    // Build the Axum router
    let app = Router::new()
        .route("/rssi", get(scan_rssi))
        .route("/location", get(get_location));

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
