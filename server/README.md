# Bluetooth RSSI Scanner Server

## Description

The **Server** is a Bluetooth Low Energy (BLE) RSSI (Received Signal Strength Indicator) scanner service built with Rust. It provides HTTP endpoints for querying RSSI values and location data of nearby Bluetooth devices.

## How It Works

1. **Neighbor Discovery**:
   - Queries blockchain and listens for blockchain events to dynamically update neighbor list
   - Adds nearby nodes (within distance of `MaxDistance`) as neighbors

2. **BLE Operations**:
   - Advertises with a unique service UUID `0000b4e7-0000-1000-8000-00805f9b34fb`
   - Scans for devices advertising the same service UUID
   - The above service UUID was chosen since it is not present in the list of assigned numbers by Bluetooth SIG

3. **RSSI Calculation**:
   - Stores up to 5 recent RSSI values per device
   - Returns median value to reduce noise from fluctuations

4. **HTTP API**:
   - Serves RSSI and location data via HTTP endpoints
   - Uses SCALE codec for compact binary serialization

## Prerequisites

### On Debian/Ubuntu:
```sh
sudo apt-get update
sudo apt-get install -y libdbus-1-dev pkg-config protobuf-compiler bluez
```

## Environment Variables

Create a `.env` file in the project root or set the following environment variables:

### Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `LATITUDE` | Your device's GPS latitude coordinate | `37.7749` |
| `LONGITUDE` | Your device's GPS longitude coordinate | `-122.4194` |

### Optional Variables

| Variable | Description | Default Value |
|----------|-------------|---------------|
| `PORT` | HTTP server listening port | `3000` |
| `RPC_URL` | Substrate node WebSocket URL | `ws://127.0.0.1:9944` |
| `BLUETOOTH_ADDRESSES` | Comma-separated list of neighbor Bluetooth addresses (for testing only) | _(empty)_ |

### Example `.env` file:

```env
# Required - Your device location
LATITUDE=37.7749
LONGITUDE=-122.4194

# Optional - Server configuration
PORT=3000
RPC_URL=ws://127.0.0.1:9944

# Optional - Manual neighbor addresses (for testing only)
# BLUETOOTH_ADDRESSES=AA:BB:CC:DD:EE:FF,11:22:33:44:55:66
```

**Note**: The `BLUETOOTH_ADDRESSES` variable is primarily for testing. In production, neighbors are automatically discovered based on GPS distance calculations and blockchain events.

## Building

### Native Build (x86_64 Linux)

```sh
cargo build --release --package server
```

The compiled binary will be located at:
```
target/release/server
```

### Cross-Compilation for ARM64/AArch64

For deploying to ARM64 devices (e.g., Raspberry Pi 4, NVIDIA Jetson), use the provided build script:

```sh
./build-server-aarch64.sh
```

The compiled ARM64 binary will be located at:
```
target/aarch64-unknown-linux-gnu/release/server
```

**Prerequisites for cross-compilation**:
```sh
cargo install cross
```

## Running

### 1. Ensure BlueZ is running

```sh
sudo systemctl status bluetooth
```

If not running:
```sh
sudo systemctl start bluetooth
```

### 2. Set environment variables

Create a `.env` file (see Environment Variables section above).

### 3. Run the server

#### Native:
```sh
./target/release/server
```

#### ARM64 (on the target device):
```sh
./target/aarch64-unknown-linux-gnu/release/server
```

**Note**: Bluetooth operations may require elevated privileges:
```sh
sudo ./target/release/server
```

## License

See the [LICENSE](/LICENSE) file in the project root.
