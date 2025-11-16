# Proof of Location Pallet

A FRAME pallet that enables decentralized proof-of-location verification through Bluetooth RSSI (Received Signal Strength Indicator) measurements and geographic proximity validation on a Substrate blockchain.

## Overview

This pallet provides a decentralized system for nodes to register their location, publish signal strength measurements to nearby neighbors, and calculate trust scores based on the accuracy of reported RSSI data compared to expected values derived from GPS coordinates.

### Key Features

- **Node Registration**: Nodes register with a unique Bluetooth MAC address and GPS coordinates
- **RSSI Data Publishing**: Nodes report signal strength measurements from nearby neighbors
- **Distance Validation**: Automatic verification that nodes are within configured maximum distance before storing RSSI data
- **Offchain Worker Integration**: Automatic fetching of location and RSSI data from external servers
- **Flexible Configuration**: Per-node server URL configuration stored on-chain
- **Node Management**: Support for updating and unregistering nodes
- **Trust Score Calculation**: RPC methods to compute trust scores by comparing measured vs. estimated RSSI values

## Configuration

The pallet requires the following configuration constants:

```rust
type ServerUrl: Get<&'static [u8]>;        // Default server URL with port
type ReferenceRssi: Get<i16>;              // Reference RSSI value at 1 meter distance
type PathLossExponent: Get<u8>;            // Path loss exponent * 10 (e.g., 4.0 → 40)
type MaxDistance: Get<u32>;                // Maximum allowed distance between nodes (meters)
type UpdateCooldown: Get<BlockNumberFor<Self>>; // Minimum blocks between node info updates
```

## Building and Testing

### Build

```bash
cargo build --package pallet-proof-of-location
```

### Test

```bash
cargo test --package pallet-proof-of-location --features runtime-benchmarks
```

### Documentation

```bash
cargo doc --package pallet-proof-of-location --open
```

## Integration

To integrate this pallet into your runtime:

1. Add to `Cargo.toml`:
```toml
pallet-proof-of-location = { path = "../pallets/proof-of-location", default-features = false }
```

2. Configure constants in your runtime:
```rust
parameter_types! {
    pub const ServerUrl: &'static [u8] = b"localhost:3000";
    pub const ReferenceRssi: i16 = -48;
    pub const PathLossExponent: u8 = 40; // 4.0 * 10
    pub const MaxDistance: u32 = 10; // 10 meters
    pub const UpdateCooldown: BlockNumber = 86400; // 1 day at 1 block/second
}
```

3. Implement the Config trait in your runtime:
```rust
impl pallet_proof_of_location::Config for Runtime {
    type AuthorityId = pallet_proof_of_location::crypto::TestAuthId;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_proof_of_location::weights::SubstrateWeight<Runtime>;
    type ServerUrl = ServerUrl;
    type ReferenceRssi = ReferenceRssi;
    type PathLossExponent = PathLossExponent;
    type MaxDistance = MaxDistance;
    type UpdateCooldown = UpdateCooldown;
}
```

4. Add to `construct_runtime!` macro:
```rust
ProofOfLocation: pallet_proof_of_location,
```

## GPS Coordinate Format

Coordinates use fixed-point precision:
- Store as `i64` values
- Multiply actual lat/lon by 1,000,000
- Example: `37.7749` → `37774900`

This preserves 6 decimal places (~0.11 meter precision) without floating point operations.

## Bluetooth Address Format

Bluetooth MAC addresses are stored as 6-byte arrays:
```rust
[u8; 6]  // Example: [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]
```

## Architecture

The pallet consists of several key components:

### Storage Items

1. **RssiData**: Stores RSSI measurements indexed by block number, neighbor account, and reporting account
2. **AccountData**: Maps AccountIds to their location data (Bluetooth address, GPS coordinates, and last update block)
3. **AddressRegistrationData**: Maps Bluetooth MAC addresses to AccountIds for quick lookups
4. **ServerConfig**: Stores per-node server configuration (hostname:port) for offchain worker data fetching

### Dispatchable Functions

#### 1. `set_server_config(origin, server_url)`
Configure the server endpoint for the offchain worker to fetch data from.

**Parameters:**
- `origin`: Must be signed by the account
- `server_url`: The full server URL with port (e.g., "localhost:3000", "192.168.1.100:8080")

#### 2. `register_node(origin, address, latitude, longitude)`
Register a node with its Bluetooth address and GPS coordinates.

**Parameters:**
- `origin`: Must be signed by the registering account
- `address`: 6-byte Bluetooth MAC address
- `latitude`: Latitude coordinate (multiply by 1,000,000 for precision)
- `longitude`: Longitude coordinate (multiply by 1,000,000 for precision)

**Errors:**
- `BluetoothAddressAlreadyTaken`: The Bluetooth address is already registered
- `AccountAlreadyRegistered`: The account has already registered a node

#### 3. `unregister_node(origin)`
Remove a node from the network, cleaning up all associated data.

**Parameters:**
- `origin`: Must be signed by the account that registered the node

**Errors:**
- `AccountNotRegistered`: The account is not registered as a node

#### 4. `update_node_info(origin, address, latitude, longitude)`
Update a registered node's Bluetooth address and/or GPS coordinates.

**Parameters:**
- `origin`: Must be signed by the account that registered the node
- `address`: New 6-byte Bluetooth MAC address
- `latitude`: New latitude coordinate (multiply by 1,000,000 for precision)
- `longitude`: New longitude coordinate (multiply by 1,000,000 for precision)

**Errors:**
- `AccountNotRegistered`: The account is not registered as a node
- `BluetoothAddressAlreadyTaken`: The new Bluetooth address is already taken
- `NodeUpdateCooldownNotElapsed`: Cooldown period has not elapsed since last update

**Note:** Updates are subject to a cooldown period (configured via `UpdateCooldown`) to prevent frequent changes. The cooldown is tracked using the `last_updated` field in location data.

#### 5. `publish_rssi_data(origin, neighbor, rssi)`
Publish RSSI measurement for a neighboring node.

**Parameters:**
- `origin`: Must be signed by the reporting node's account
- `neighbor`: The AccountId of the neighboring node being measured
- `rssi`: The signal strength measurement (i16, typically negative dBm values)

**Errors:**
- `AccountNotRegistered`: Either the reporting node or neighbor is not registered
- `ExceedsMaxDistance`: The distance between nodes exceeds the configured maximum

### Events

1. **RssiStored**: Emitted when RSSI data is successfully stored
2. **NodeRegistered**: Emitted when a new node is registered
3. **NodeUnregistered**: Emitted when a node is unregistered
4. **NodeUpdated**: Emitted when a node's information is updated

### Offchain Worker

The offchain worker automatically:
1. Fetches location data from configured server endpoints
2. Fetches RSSI measurements from nearby devices
3. Submits signed transactions to register nodes and publish RSSI data
4. Runs on each new block when the node is fully synced

### Runtime API & RPC

The pallet provides RPC methods for trust score calculation:

#### 1. `calculate_trust_score(target_block, account)`
Calculate trust score for a specific account at a given block number.

**Parameters:**
- `target_block`: The block number to calculate trust score for
- `account`: The account to calculate trust score for

**Returns:** Trust score error value (lower is better), or None if no data available

#### 2. `calculate_trust_scores(target_block)`
Calculate trust scores for all accounts at a given block number.

**Parameters:**
- `target_block`: The block number to calculate trust scores for

**Returns:** Vector of (AccountId, trust_score) tuples for all accounts

### Trust Score Calculation

The trust score is calculated using:
1. **RSSI Estimation**: Based on GPS coordinates using path loss model: `RSSI = r - n * 10 * log10(d)`
   - `r`: Reference RSSI at 1 meter (configured via `ReferenceRssi`)
   - `n`: Path loss exponent (configured via `PathLossExponent`, divided by 10)
   - `d`: Distance calculated from GPS coordinates using Haversine formula
   - More details in the [measurements folder](/measurements).
2. **Error Calculation**: Difference between measured and estimated RSSI values
3. **Trimmed Median**: Discards highest 25% of errors and returns median of remaining values

Lower trust scores indicate more accurate RSSI reporting.

## Security Considerations

- Nodes can only update/unregister their own data
- Bluetooth addresses must be unique across the network
- Distance validation prevents nodes from reporting RSSI for distant neighbors
- Fixed-point arithmetic avoids floating-point non-determinism in consensus
- Update cooldown mechanism prevents frequent node information changes, improving data stability
- Location data includes `last_updated` timestamp to track when information was last modified

## License

Unlicense
