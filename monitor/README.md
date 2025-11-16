# Trust Score Monitor

## Description

The **Monitor** is a GUI application that provides real-time visualization of trust score errors for blockchain nodes. It displays error values in an interactive bar chart, updating automatically as new blocks are finalized.

## User Interface

![Monitor UI animation](/assets/monitor.gif)

## How It Works

1. **Blockchain Connection**:
   - Connects to a Substrate node via WebSocket
   - Subscribes to finalized blocks (starting from block #3)
   - Calls the `calculate_trust_scores` runtime API for each new block

2. **Data Processing**:
   - Retrieves trust score error values for all nodes
   - Maps account IDs to friendly names (Alice, Bob, Charlie, etc.)
   - Updates the GUI in real-time with the latest error data

3. **Visualization**:
   - Displays error values as an interactive bar chart
   - Y-axis fixed from 0 to 10 (error range)
   - X-axis labeled with account names
   - Shows current block number in the title
   - Auto-refreshes as new blocks arrive

## Prerequisites

### On Debian/Ubuntu:
```sh
sudo apt-get update
sudo apt-get install -y protobuf-compiler
```

## Environment Variables

All environment variables are **optional**. The monitor works with defaults.

| Variable | Description | Default Value |
|----------|-------------|---------------|
| `RPC_URL` | Substrate node WebSocket URL | `ws://127.0.0.1:9944` |

### Example `.env` file:

```env
# Optional - Node configuration
RPC_URL=ws://127.0.0.1:9944
```

## Building

### Native Build (x86_64 Linux)

```sh
cargo build --release --package monitor
```

The compiled binary will be located at:
```
target/release/monitor
```

## Running

### 1. Ensure the blockchain node is running

The monitor requires a running Substrate node with the trust score API:

```sh
./target/release/solochain-template-node --dev
```

### 2. Run the monitor

#### Native:
```sh
./target/release/monitor
```

Or using cargo:
```sh
cargo run --package monitor --release
```

The GUI window will open and begin displaying trust score data once blocks start finalizing.

## License

See the [LICENSE](/LICENSE) file in the project root.
