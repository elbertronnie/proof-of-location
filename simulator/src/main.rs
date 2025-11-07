use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

const ALICE_NODE_ID: &str = "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d";
const ALICE_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:01";

const BOB_NODE_ID: &str = "0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48";
const BOB_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:02";
const BOB_LATITUDE: f64 = 0.00001;
const BOB_LONGITUDE: f64 = 0.0;

const CHARLIE_NODE_ID: &str = "0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22";
const CHARLIE_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:03";
const CHARLIE_LATITUDE: f64 = -0.00001;
const CHARLIE_LONGITUDE: f64 = 0.0;

const DAVE_NODE_ID: &str = "0x306721211d5404bd9da88e0204360a1a9ab8b87c66c1bc2fcdd37f3c2222cc20";
const DAVE_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:04";
const DAVE_LATITUDE: f64 = 0.0;
const DAVE_LONGITUDE: f64 = 0.00001;

const EVE_NODE_ID: &str = "0xe659a7a1628cdd93febc04a4e0646ea20e9f5f0ce097d9a05290d4a9e054df4e";
const EVE_BLUETOOTH_ADDRESS: &str = "AA:BB:CC:DD:EE:05";
const EVE_LATITUDE: f64 = 0.0;
const EVE_LONGITUDE: f64 = -0.00001;

const PATH_LOSS_EXPONENT: f64 = 3.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlicePosition {
    latitude: f64,
    longitude: f64,
}
type SharedState = Arc<RwLock<AlicePosition>>;

#[derive(Encode, Decode, Debug, Clone)]
struct DeviceRssi {
    address: [u8; 6],
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

fn estimate_rssi(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> i16 {
    use haversine_redux::Location;
    use rand::{thread_rng, Rng};
    use rand_distr::Normal;

    let a = Location::new(a_lat, a_lon);
    let b = Location::new(b_lat, b_lon);
    let dist = a.kilometers_to(&b) * 1000.0; // convert kilometers to meters
    let rssi = -60.0 - PATH_LOSS_EXPONENT * 10.0 * dist.log10();
    let noise = thread_rng().sample(Normal::new(0.0, 2.0).unwrap());
    (rssi + noise) as i16
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

async fn get_node_info(
    node_id: &str,
    state: &SharedState,
) -> Option<(String, &'static str, f64, f64)> {
    match node_id {
        ALICE_NODE_ID => {
            let alice_pos = state.read().await;
            Some((
                "Alice".to_string(),
                ALICE_BLUETOOTH_ADDRESS,
                alice_pos.latitude,
                alice_pos.longitude,
            ))
        }
        BOB_NODE_ID => Some((
            "Bob".to_string(),
            BOB_BLUETOOTH_ADDRESS,
            BOB_LATITUDE,
            BOB_LONGITUDE,
        )),
        CHARLIE_NODE_ID => Some((
            "Charlie".to_string(),
            CHARLIE_BLUETOOTH_ADDRESS,
            CHARLIE_LATITUDE,
            CHARLIE_LONGITUDE,
        )),
        DAVE_NODE_ID => Some((
            "Dave".to_string(),
            DAVE_BLUETOOTH_ADDRESS,
            DAVE_LATITUDE,
            DAVE_LONGITUDE,
        )),
        EVE_NODE_ID => Some((
            "Eve".to_string(),
            EVE_BLUETOOTH_ADDRESS,
            EVE_LATITUDE,
            EVE_LONGITUDE,
        )),
        _ => None,
    }
}

async fn get_all_nodes(state: &SharedState) -> Vec<(&'static str, String, &'static str, f64, f64)> {
    let alice_pos = state.read().await;
    vec![
        (
            ALICE_NODE_ID,
            "Alice".to_string(),
            ALICE_BLUETOOTH_ADDRESS,
            alice_pos.latitude,
            alice_pos.longitude,
        ),
        (
            BOB_NODE_ID,
            "Bob".to_string(),
            BOB_BLUETOOTH_ADDRESS,
            BOB_LATITUDE,
            BOB_LONGITUDE,
        ),
        (
            CHARLIE_NODE_ID,
            "Charlie".to_string(),
            CHARLIE_BLUETOOTH_ADDRESS,
            CHARLIE_LATITUDE,
            CHARLIE_LONGITUDE,
        ),
        (
            DAVE_NODE_ID,
            "Dave".to_string(),
            DAVE_BLUETOOTH_ADDRESS,
            DAVE_LATITUDE,
            DAVE_LONGITUDE,
        ),
        (
            EVE_NODE_ID,
            "Eve".to_string(),
            EVE_BLUETOOTH_ADDRESS,
            EVE_LATITUDE,
            EVE_LONGITUDE,
        ),
    ]
}

async fn scan_rssi(State(state): State<SharedState>, req: Request) -> impl IntoResponse {
    let node_id = req
        .headers()
        .get("X-Node-ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    println!("📡 RSSI request from node: {}", node_id);
    let (_, _, requester_lat, requester_lon) = match get_node_info(node_id, &state).await {
        Some(info) => info,
        None => {
            let error_msg = format!("Unknown node ID: {}", node_id);
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(error_msg))
                .unwrap();
        }
    };
    let mut devices = Vec::new();
    for (other_node_id, name, bluetooth_addr_str, other_lat, other_lon) in
        get_all_nodes(&state).await
    {
        if other_node_id == node_id {
            continue;
        }
        let address = match parse_bluetooth_address(bluetooth_addr_str) {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("Failed to parse Bluetooth address for {}: {}", name, e);
                continue;
            }
        };
        let rssi = estimate_rssi(requester_lat, requester_lon, other_lat, other_lon);
        devices.push(DeviceRssi { address, rssi });
        println!("  {} ({}): RSSI = {} dBm", name, bluetooth_addr_str, rssi);
    }
    println!("Returning RSSI data for {} devices\n", devices.len());
    let response = RssiResponse { devices };
    let encoded = response.encode();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(encoded))
        .unwrap()
}

async fn get_location(State(state): State<SharedState>, req: Request) -> impl IntoResponse {
    let node_id = req
        .headers()
        .get("X-Node-ID")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    println!("📍 Location request from node: {}", node_id);
    let (_, bluetooth_address_str, latitude, longitude) = match get_node_info(node_id, &state).await
    {
        Some(info) => info,
        None => {
            let error_msg = format!("Unknown node ID: {}", node_id);
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(error_msg))
                .unwrap();
        }
    };
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
    let encoded = response.encode();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(encoded))
        .unwrap()
}

async fn update_alice_position(
    State(state): State<SharedState>,
    Json(new_pos): Json<AlicePosition>,
) -> impl IntoResponse {
    let mut alice_pos = state.write().await;
    *alice_pos = new_pos.clone();
    println!(
        "🔄 Updated Alice's position to: lat={}, lon={}",
        new_pos.latitude, new_pos.longitude
    );
    Json(new_pos)
}

async fn get_positions(State(state): State<SharedState>) -> impl IntoResponse {
    let alice_pos = state.read().await;
    #[derive(Serialize)]
    struct NodePosition {
        name: String,
        latitude: f64,
        longitude: f64,
        color: String,
    }
    let positions = vec![
        NodePosition {
            name: "Alice".to_string(),
            latitude: alice_pos.latitude,
            longitude: alice_pos.longitude,
            color: "#e74c3c".to_string(),
        },
        NodePosition {
            name: "Bob".to_string(),
            latitude: BOB_LATITUDE,
            longitude: BOB_LONGITUDE,
            color: "#3498db".to_string(),
        },
        NodePosition {
            name: "Charlie".to_string(),
            latitude: CHARLIE_LATITUDE,
            longitude: CHARLIE_LONGITUDE,
            color: "#2ecc71".to_string(),
        },
        NodePosition {
            name: "Dave".to_string(),
            latitude: DAVE_LATITUDE,
            longitude: DAVE_LONGITUDE,
            color: "#f39c12".to_string(),
        },
        NodePosition {
            name: "Eve".to_string(),
            latitude: EVE_LATITUDE,
            longitude: EVE_LONGITUDE,
            color: "#9b59b6".to_string(),
        },
    ];
    Json(positions)
}

async fn serve_ui() -> Html<&'static str> {
    Html(include_str!("ui.html"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting Location Simulator Server...\n");
    let state = Arc::new(RwLock::new(AlicePosition {
        latitude: 0.00001,
        longitude: 0.00001,
    }));
    let app = Router::new()
        .route("/", get(serve_ui))
        .route("/rssi", get(scan_rssi))
        .route("/location", get(get_location))
        .route("/api/update-alice", post(update_alice_position))
        .route("/api/positions", get(get_positions))
        .with_state(state);
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    println!("Server listening on http://{}", addr);
    println!(
        "🌐 Open http://{} in your browser to access the interactive map",
        addr
    );
    println!("📡 RSSI endpoint: http://{}/rssi", addr);
    println!("📍 Location endpoint: http://{}/location\n", addr);
    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
