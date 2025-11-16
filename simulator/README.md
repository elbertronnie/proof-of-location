# Bluetooth RSSI Server Simulator

## Description

This **Simulator** is a testing tool that simulates 5 Bluetooth-enabled server nodes (Alice, Bob, Charlie, Dave, and Eve) for development and testing purposes. It mocks the behavior of multiple [server](../server) instances without requiring actual Bluetooth hardware or multiple physical devices.

## User Interface

![Simulator UI animation](/assets/simulator.gif)

## How It Works

1. **Web UI Interaction**:
   - User drags Alice on the interactive map
   - Frontend sends POST request to `/api/update-alice`
   - Backend updates Alice's position in shared state
   - All subsequent RSSI calculations use the new position
   - UI polls `/api/positions` to refresh the visualization

2. **RSSI Requests**:
   - Receives request with `X-Node-ID` header identifying the requester
   - Calculates distance from requester to all other nodes using Haversine formula
   - Applies log-distance path loss model to estimate RSSI
   - Adds Gaussian noise (σ = 2 dBm) for realism
   - Returns SCALE-encoded response with all RSSI values

3. **Location Requests**:
   - Looks up the requesting node's Bluetooth address and coordinates
   - Returns SCALE-encoded response with node's location data

4. **Path Loss Simulation**:
   - Uses standard path loss model to calculate RSSI
   - Signal strength decreases logarithmically with distance
   - Random noise simulates environmental interference

## Prerequisites

### On Debian/Ubuntu:
```sh
sudo apt-get update
sudo apt-get install -y protobuf-compiler
```

## Environment Variables

All environment variables are **optional**. The simulator works out-of-the-box with defaults.

| Variable | Description | Default Value |
|----------|-------------|---------------|
| `PORT` | HTTP server listening port | `3000` |

### Example `.env` file:

```env
# Optional - Server configuration
PORT=3000
```

## Building

### Native Build (x86_64 Linux)

```sh
cargo build --release --package simulator
```

The compiled binary will be located at:
```
target/release/simulator
```

## Running

### 1. Run the simulator

#### Native:
```sh
./target/release/simulator
```

Or using cargo:
```sh
cargo run --package simulator --release
```

### 2. Open the web interface

Navigate to `http://localhost:3000` in your browser to access the interactive map where you can:
- See all 5 nodes visualized on a coordinate grid
- Drag Alice (red node) to different positions
- Watch RSSI values update as positions change

## Differences from Real Server

The simulator differs from the [real server](../server) in the following ways:

| Feature | Simulator | Real Server |
|---------|-----------|-------------|
| **Bluetooth** | No hardware required | Requires BlueZ and Bluetooth adapter |
| **RSSI Source** | Calculated from GPS distance | Actual BLE signal strength |
| **Nodes** | 5 pre-configured mock nodes | Single physical device |
| **Blockchain** | No connection required | Connects to Substrate node |
| **Position Updates** | Via web UI / API | Fixed from environment variables |
| **Use Case** | Testing and development | Production deployment |

## Mock Nodes Configuration

The simulator provides 5 pre-configured nodes:

| Node | Node ID (Account) | Bluetooth Address | Initial Latitude | Initial Longitude | UI Color |
|------|-------------------|-------------------|------------------|-------------------|----------|
| **Alice** | `0xd43593...6da27d` | `AA:BB:CC:DD:EE:01` | `0.00001` (dynamic) | `0.00001` (dynamic) | Red |
| **Bob** | `0x8eaf04...f26a48` | `AA:BB:CC:DD:EE:02` | `0.00001` | `0.0` | Blue |
| **Charlie** | `0x90b5ab...65fe22` | `AA:BB:CC:DD:EE:03` | `-0.00001` | `0.0` | Green |
| **Dave** | `0x306721...22cc20` | `AA:BB:CC:DD:EE:04` | `0.0` | `0.00001` | Orange |
| **Eve** | `0xe659a7...54df4e` | `AA:BB:CC:DD:EE:05` | `0.0` | `-0.00001` | Purple |

**Note**: Alice's position is dynamic and can be updated through the web UI or API. The other 4 nodes have fixed positions.

## RSSI Calculation

The simulator uses a **log-distance path loss model** to estimate realistic RSSI values:

```
RSSI = r - (n × 10 × log₁₀(d)) + N(0,σ)
```

Where:
- **r**: Reference RSSI at 1 meter distance
- **n**: Path loss exponent (indoor/urban environment)
- **d**: Distance between nodes in meters (calculated using Haversine formula)
- **N(0,σ)**: Gaussian noise with mean=0, standard deviation=σ

This model simulates realistic signal attenuation over distance with random fluctuations. More details in the [measurements folder](/measurements).

## License

See the [LICENSE](/LICENSE) file in the project root.
