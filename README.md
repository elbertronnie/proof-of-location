# PoLka.Blue (Proof of Location)

A decentralized blockchain-based system for verifying physical location through Bluetooth signal strength measurements. Built on [Substrate](https://substrate.io/), this project enables trustless location verification without relying on centralized authorities.

**Note:** This is a hackathon project demonstrating the concept of decentralized proof of location. In real life, LoRa is more applicable. I have used Bluetooth here for ease of prototyping with common hardware.

## Why This Matters

### The Problem

Traditional location verification systems rely on centralized authorities (GPS satellites, cell towers, Wi-Fi databases) which create single points of failure and trust. These systems can be:
- **Spoofed**: GPS signals can be faked with readily available hardware
- **Controlled**: Central authorities can manipulate or censor location data
- **Inaccessible**: May not work in certain regions or conditions

### The Solution

This project implements a **decentralized proof-of-location** system where:
- **Nodes verify each other** through Bluetooth RSSI (Received Signal Strength Indicator) measurements
- **Trust is earned** by consistently reporting accurate signal strength data
- **No central authority** controls or validates location claims
- **Blockchain immutability** creates an auditable history of location proofs

### Use Cases

- **Decentralized ride-sharing**: Verify driver/passenger proximity without GPS spoofing
- **Supply chain**: Prove physical custody and location of goods
- **Attendance verification**: Confirm physical presence at events or workplaces
- **Geo-fenced DeFi**: Execute smart contracts based on verified location
- **IoT device authentication**: Ensure devices are where they claim to be

## User Interface

### Simulator

![Simulator UI](/assets/simulator.png)

### Monitor

![Monitor UI](/assets/monitor.png)

## Quick Start

### Prerequisites

**On Debian/Ubuntu:**
```sh
sudo apt-get update
sudo apt-get install -y build-essential git clang curl libssl-dev llvm libudev-dev \
    protobuf-compiler pkg-config libdbus-1-dev bluez
```

**Install Rust:**
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 1. Build the Blockchain

```sh
cargo build --release
```

This builds:
- `target/release/solochain-template-node` - The blockchain node
- `target/release/server` - The BLE RSSI scanner
- `target/release/simulator` - The testing simulator
- `target/release/monitor` - The trust score visualizer

**Build Issues?** If you encounter dependency issues or compilation errors, use the development container which provides a pre-configured build environment:

```sh
# Start the development container
docker-compose up -d builder

# Enter the container
docker-compose exec builder bash

# Inside the container, build the project
cargo build --release

# Exit the container
exit

# Stop the container when done
docker-compose down
```

The development container includes all necessary dependencies (Rust toolchain, protobuf compiler, build tools, etc.) and ensures a consistent build environment across different systems.

### 2. Run the Blockchain

```sh
zombienet spawn zombienet.toml --provider native
```

This command will start a chain with 5 nodes.

### 3. Run the Data Collection Server

**Environment Variables**

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `LATITUDE` | Your device's GPS latitude coordinate | Yes | - |
| `LONGITUDE` | Your device's GPS longitude coordinate | Yes | - |
| `PORT` | HTTP server listening port | No | `3000` |
| `RPC_URL` | Substrate node WebSocket URL | No | `ws://127.0.0.1:9944` |

**Option A: Real Hardware**

Create `.env` file:
```env
LATITUDE=37.7749
LONGITUDE=-122.4194
PORT=3000
RPC_URL=ws://127.0.0.1:9944
```

Run:
```sh
sudo ./target/release/server
```

**Option B: Real ARM64 Hardware (Raspberry Pi)**

Build the ARM64 binary:
```sh
./build-server-aarch64.sh
```

Create `.env` file:
```env
LATITUDE=37.7749
LONGITUDE=-122.4194
PORT=3000
RPC_URL=ws://127.0.0.1:9944
```

Run:
```sh
sudo ./target/aarch64-unknown-linux-gnu/release/server
```

**Option C: Simulator (no Bluetooth needed)**

```sh
./target/release/simulator
```

Then open `http://localhost:3000` to interact with the simulation.

### 4. Set Server Config

If you have chosen **Option A** or **Option B** for the server, you will need to specify the url of your server to the blockchain.

Use the configuration script:
```sh
./configure-url-port.sh <RPC_URL> <ACCOUNT> <SERVER_URL>
```

Examples:
```sh
# Configure Alice's node (default validator on port 9944)
./configure-url-port.sh ws://localhost:9944 //Alice localhost:3000

# Configure Bob's node (validator on port 9945)
./configure-url-port.sh ws://localhost:9945 //Bob localhost:3001

# Configure with IP address
./configure-url-port.sh ws://localhost:9946 //Charlie 192.168.1.100:3002
```

**Manual Configuration via Polkadot.js:**

If you prefer to configure manually:
1. Open [Polkadot.js Apps](https://polkadot.js.org/apps/?rpc=ws://127.0.0.1:9944)
2. Navigate to **Developer** → **Extrinsics**
3. Select the account (e.g., Alice)
4. Choose extrinsic: `proofOfLocation` → `setServerConfig(server_url)`
5. Enter your server URL
6. Submit the transaction

### 5. Monitor Trust Scores

```sh
./target/release/monitor
```

A GUI window opens showing real-time trust scores for all nodes.

## Development

### Run Tests

```sh
# All tests
cargo test

# Specific package
cargo test --package pallet-proof-of-location --features runtime-benchmarks
```

### Generate Documentation

```sh
cargo doc --open
```

## Configuration

Key pallet parameters (configured in [`runtime/src/lib.rs`](./runtime/src/lib.rs)):

| Parameter | Description | Default |
|-----------|-------------|---------|
| `ReferenceRssi` | RSSI at 1 meter distance | -48 dBm |
| `PathLossExponent` | Signal attenuation rate (×10) | 40 (= 4.0) |
| `MaxDistance` | Maximum neighbor distance | 10 meters |
| `UpdateCooldown` | Minimum blocks between updates | 86400 blocks |

## How It Works

### Architecture Overview

![Architechture Diagram](/assets/architecture.svg)

### Core Mechanism

1. **Registration**: Each node registers its Bluetooth MAC address and GPS coordinates on-chain

2. **RSSI Collection**: Nodes continuously measure Bluetooth signal strength from nearby neighbors and publish this data to the blockchain

3. **Distance Validation**: The system calculates expected distance between nodes using GPS coordinates (Haversine formula) and rejects measurements from nodes too far apart

4. **Trust Score Calculation**: 
   - For each RSSI measurement, calculate the **expected** signal strength using GPS distance and the log-distance path loss model:
     ```
     RSSI_expected = r - n × 10 × log₁₀(d)
     ```
     Where:
     - `r` = reference RSSI at 1 meter
     - `n` = path loss exponent (environmental factor)
     - `d` = GPS-calculated distance in meters
   
   - Compare **measured** vs **expected** RSSI to calculate error
   - Aggregate errors over multiple measurements using trimmed median (discarding worst 25%)
   - Lower error = higher trust (honest reporting)

5. **Reputation**: Nodes that consistently report accurate RSSI measurements build trust scores, while dishonest nodes accumulating high errors can be identified and excluded

## Project Structure

### 🔗 Blockchain Components

#### [`pallets/proof-of-location/`](./pallets/proof-of-location/)
The core Substrate pallet implementing:
- Node registration with Bluetooth addresses and GPS coordinates
- RSSI data storage and validation
- Distance-based proximity checks
- Offchain worker for automated data collection
- Runtime APIs for trust score calculation

**See [pallet documentation](./pallets/proof-of-location/README.md)**

#### [`runtime/`](./runtime/)
The blockchain runtime that integrates the proof-of-location pallet with Substrate's core pallets (Balances, Timestamp, GRANDPA, Aura, etc.)

#### [`node/`](./node/)
The blockchain node implementation that:
- Runs the consensus mechanism (Aura + GRANDPA)
- Exposes RPC endpoints for querying trust scores
- Manages peer-to-peer networking
- Executes offchain workers

### 📡 Other Components

#### [`server/`](./server/)
Bluetooth RSSI scanner that runs on each physical device to:
- Advertise its presence via BLE
- Scan for nearby devices
- Measure and store RSSI values
- Provide HTTP API for the offchain worker to fetch data
- Automatically discover neighbors from the blockchain

Supports both x86_64 and ARM64 (Raspberry Pi, etc.)

**See [server documentation](./server/README.md)**

#### [`simulator/`](./simulator/)
Testing tool that simulates 5 virtual Bluetooth nodes (Alice, Bob, Charlie, Dave, Eve) with:
- Interactive web UI for moving nodes
- Realistic RSSI calculation based on distance
- Gaussian noise simulation
- No Bluetooth hardware required

Perfect for development and testing without physical devices.

**See [simulator documentation](./simulator/README.md)**

#### [`monitor/`](./monitor/)
Real-time GUI visualization showing:
- Trust score errors for all nodes
- Updates on each new block
- Interactive bar charts
- Block-by-block history

**See [monitor documentation](./monitor/README.md)**

#### [`measurements/`](./measurements/)
Real-world RSSI measurement data and analysis:
- RSSI measurements collected at different distances (3, 6, and 12 steps)
- Jupyter notebook for analyzing reference RSSI, path loss exponent, and noise distribution
- Used to calibrate and validate the trust score algorithm

The analysis helps determine optimal values for:
- Reference RSSI at 1 meter
- Path loss exponent for the environment
- Expected noise characteristics

## Technical Details

### Trust Score Algorithm

1. For each neighbor's RSSI measurement:
   a. Calculate GPS distance using Haversine formula
   b. Estimate RSSI: r - n × 10 × log₁₀(distance)
   c. Error = |measured_RSSI - estimated_RSSI|

2. Collect all errors for a node

3. Proceed if number of errors > 3, else return None (insufficient data)

4. Remove worst 25% (trimmed)

5. Return median of remaining errors

Lower errors indicate more trustworthy nodes.

### Why Trimmed Median?

- **Robust to outliers**: Environmental interference can cause occasional bad readings
- **Prevents gaming**: A few accurate measurements can't mask systematic dishonesty
- **Fair**: Removes worst-case errors that may be beyond node's control

### Why greater than 3?

- **3 distances = unique location**: 3 distance measurements from 3 known points uniquely determine a position on Earth (trilateration principle)
- **GPS analogy**: GPS requires at least 3 satellites visible to calculate position
- **Fault tolerance**: Requiring > 3 (not = 3) accounts for one potentially misbehaving or compromised node

## Security Considerations

✅ **Sybil Resistance**: Trust scores make it expensive to create fake nodes with fabricated locations

✅ **Collusion Detection**: Cross-validation between multiple nodes reveals coordinated lying

✅ **Distance Validation**: Prevents distant nodes from claiming proximity

⚠️ **Bluetooth Spoofing**: Advanced attackers with SDR equipment could fake BLE packets

⚠️ **Environmental Factors**: Buildings, weather, and interference affect RSSI accuracy

## License

Unlicense (see [LICENSE](./LICENSE))

## Acknowledgments

I would like to thank the organizers of the Polkadot Cloud hackathon for providing the opportunity to create a Blockchain. Special thanks to the Substrate and Polkadot communities for their invaluable resources and support. I would also like to thank my friends who provided me with their Raspberry Pi for testing and helped in recording the physical demo. 

This project was submitted to the **Build Resilient Apps with Polkadot Cloud** hackathon. The presentation is available [here](https://docs.google.com/presentation/d/1EBB28O8JHHKbdNopGoujANowPjOCaK3CN_OvF_tEO2Y/edit?usp=sharing). The video is available [here](https://youtu.be/sdNUI49zW_4). This project was built with the following technologies:

- [Substrate](https://substrate.io/)
- [Polkadot SDK](https://github.com/paritytech/polkadot-sdk)
- [Subxt](https://github.com/paritytech/subxt) for Substrate client interactions
- [BlueZ](http://www.bluez.org/) for Bluetooth functionality
- [egui](https://github.com/emilk/egui) for GUI development
- [axum](https://github.com/tokio-rs/axum) for HTTP server
