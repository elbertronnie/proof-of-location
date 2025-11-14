//! # Proof of Location Pallet
//!
//! A FRAME pallet that enables decentralized proof-of-location verification through
//! Bluetooth RSSI measurements and geographic proximity validation.
//!
//! ## Overview
//!
//! This pallet provides functionality for:
//! - Registering nodes with Bluetooth addresses and GPS coordinates
//! - Publishing RSSI (Received Signal Strength Indicator) data between neighboring nodes
//! - Validating proximity between nodes using distance calculations
//! - Configuring per-node server endpoints for offchain data fetching
//! - Automatic node registration via offchain workers
//! - RPC endpoints for calculating trust scores based on RSSI accuracy
//!
//! Each pallet section is annotated with an attribute using the `#[pallet::...]` procedural macro.
//! This macro generates the necessary code for a pallet to be aggregated into a FRAME runtime.
//!
//! Learn more about FRAME macros [here](https://docs.substrate.io/reference/frame-macros/).
//!
//! ### Key Features
//!
//! - **Node Registration**: Nodes register with a unique Bluetooth MAC address and GPS coordinates
//! - **RSSI Data Publishing**: Nodes report signal strength measurements from nearby neighbors
//! - **Distance Validation**: Automatic verification that nodes are within configured maximum distance
//! - **Offchain Worker Integration**: Automatic fetching of location and RSSI data from external servers
//! - **Flexible Configuration**: Per-node server URL configuration stored on-chain
//! - **Node Management**: Support for updating and unregistering nodes
//! - **Trust Score Calculation**: RPC methods to compute trust scores by comparing measured vs. estimated RSSI
//!
//! ### Pallet Sections
//!
//! - **Configuration trait** ([`Config`]): Defines the types, constants (server URL, max distance), and crypto requirements
//! - **Storage items**: RssiData, AccountData, AddressRegistrationData, ServerConfig
//! - **Events** ([`Event`]): RssiStored, NodeRegistered, NodeUnregistered, NodeUpdated
//! - **Errors** ([`Error`]): Address/account validation and distance verification errors
//! - **Dispatchable functions**: set_server_config, register_node, unregister_node, update_node_info, publish_rssi_data
//! - **Offchain worker**: Automatic location registration and RSSI data submission
//! - **RPC methods**: calculate_trust_score (for specific account), calculate_all_trust_scores (for all accounts)
//!
//! Run `cargo doc --package pallet-proof-of-location --open` to view this pallet's documentation.

// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

// Import alloc for format! macro in no_std
extern crate alloc;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

// Runtime API for RPC
pub mod rpc;

// Utility module containing common structs and functions
pub mod util;

// Module containing RPC implementation functions
mod rpc_impl;

// Module containing pallet calls (dispatchable functions)
mod pallet_calls;

// Module containing offchain worker implementation
mod offchain_worker;

// FRAME pallets require their own "mock runtimes" to be able to run unit tests. This module
// contains a mock runtime specific for testing this pallet's functionality.
#[cfg(test)]
mod mock;

// This module contains the unit tests for this pallet.
// Learn about pallet unit testing here: https://docs.substrate.io/test/unit-testing/
#[cfg(test)]
mod tests;

// Every callable function or "dispatchable" a pallet exposes must have weight values that correctly
// estimate a dispatchable's execution time. The benchmarking module is used to calculate weights
// for each dispatchable and generates this pallet's weight.rs file. Learn more about benchmarking here: https://docs.substrate.io/test/benchmark/
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::*;

use sp_core::crypto::KeyTypeId;

/// Defines application identifier for crypto keys of this module.
///
/// Every module that deals with signatures needs to declare its unique identifier for
/// its crypto keys.
/// When offchain worker is signing transactions it's going to request keys of type
/// `KeyTypeId` from the keystore and use the ones it finds to sign the transaction.
/// The keys can be inserted manually via RPC (see `author_insertKey`).
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"loc!");

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrappers.
/// We can use from supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment
/// the types with this pallet-specific identifier.
pub mod crypto {
    use super::KEY_TYPE;
    use sp_core::sr25519::Signature as Sr25519Signature;
    use sp_runtime::{
        app_crypto::{app_crypto, sr25519},
        traits::Verify,
        MultiSignature, MultiSigner,
    };
    app_crypto!(sr25519, KEY_TYPE);

    pub struct TestAuthId;

    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }

    // implemented for mock runtime in test
    impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
        for TestAuthId
    {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }
}

use frame_support::pallet_macros::import_section;

/// Import pallet sections from separate files
#[import_section(pallet_calls::dispatches)]
#[import_section(offchain_worker::offchain)]
// All pallet logic is defined in its own module and must be annotated by the `pallet` attribute.
#[frame_support::pallet]
pub mod pallet {
    // Import various useful types required by all FRAME pallets.
    use super::*;
    use crate::util::LocationData;
    use frame_support::pallet_prelude::*;
    use frame_system::offchain::{AppCrypto, CreateSignedTransaction};
    use frame_system::pallet_prelude::*;

    // The `Pallet` struct serves as a placeholder to implement traits, methods and dispatchables
    // (`Call`s) in this pallet.
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The pallet's configuration trait.
    ///
    /// All our types and constants a pallet depends on must be declared here.
    /// These types are defined generically and made concrete when the pallet is declared in the
    /// `runtime/src/lib.rs` file of your chain.
    #[pallet::config]
    pub trait Config: CreateSignedTransaction<Call<Self>> + frame_system::Config {
        /// The identifier type for an offchain worker.
        type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// A type representing the weights required by the dispatchables of this pallet.
        type WeightInfo: WeightInfo;

        /// Default server URL with port for fetching data (used if not set via set_server_config).
        ///
        /// Format: "hostname:port" or "ip:port" (e.g., "localhost:3000")
        #[pallet::constant]
        type ServerUrl: Get<&'static [u8]>;

        /// Reference RSSI value at 1 meter distance.
        #[pallet::constant]
        type ReferenceRssi: Get<i16>;

        /// Path loss exponent multiplied by 10.
        ///
        /// Multiplied by 10 to allow fractional values (e.g., 4.0 -> 40).
        #[pallet::constant]
        type PathLossExponent: Get<u8>;

        /// Maximum allowed distance between 2 nodes (in meters) to consider publishing RSSI data.
        #[pallet::constant]
        type MaxDistance: Get<u32>;

        /// Minimum number of blocks that must elapse before a node can update its information again.
        #[pallet::constant]
        type UpdateCooldown: Get<BlockNumberFor<Self>>;
    }

    /// Storage for RSSI (Received Signal Strength Indicator) measurements.
    ///
    /// Maps (block_number, neighbor_account, reporting_account) -> RSSI value (i16)
    /// This allows tracking signal strength measurements over time between node pairs.
    #[pallet::storage]
    pub type RssiData<T: Config> = StorageNMap<
        Key = (
            NMapKey<Identity, BlockNumberFor<T>>,
            NMapKey<Blake2_128Concat, T::AccountId>, // neighbor account
            NMapKey<Blake2_128Concat, T::AccountId>, // reporting account
        ),
        Value = i16,
    >;

    /// Maps Bluetooth MAC addresses to AccountIds.
    ///
    /// Used to look up which account owns a particular Bluetooth address,
    /// enabling RSSI data to reference neighbors by their MAC addresses.
    #[pallet::storage]
    pub type AddressRegistrationData<T: Config> =
        StorageMap<Hasher = Blake2_128Concat, Key = [u8; 6], Value = T::AccountId>;

    /// Maps AccountIds to their location data (Bluetooth address + GPS coordinates).
    ///
    /// Stores the registered location information for each node in the network.
    #[pallet::storage]
    pub type AccountData<T: Config> =
        StorageMap<Hasher = Blake2_128Concat, Key = T::AccountId, Value = LocationData>;

    /// Storage for server configuration per account (node)
    /// Maps AccountId -> server URL (format: "hostname:port" or "ip:port")
    #[pallet::storage]
    pub type ServerConfig<T: Config> = StorageMap<
        Hasher = Blake2_128Concat,
        Key = T::AccountId,
        Value = BoundedVec<u8, ConstU32<256>>,
    >;

    /// Events that functions in this pallet can emit.
    ///
    ///	The `generate_deposit` macro generates a function on `Pallet` called `deposit_event` which
    /// will convert the event type of your pallet into `RuntimeEvent` (declared in the pallet's
    /// [`Config`] trait) and deposit it using [`frame_system::Pallet::deposit_event`].
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A user has successfully published RSSI of its neighbor.
        RssiStored {
            block_number: BlockNumberFor<T>,
            neighbor: T::AccountId,
            who: T::AccountId,
            rssi: i16,
        },
        /// A node has successfully registered its location.
        NodeRegistered {
            address: [u8; 6],
            who: T::AccountId,
            latitude: i64,
            longitude: i64,
        },
        /// A node has been unregistered.
        NodeUnregistered { address: [u8; 6], who: T::AccountId },
        /// A node's information has been updated.
        NodeUpdated {
            who: T::AccountId,
            old_address: [u8; 6],
            new_address: [u8; 6],
            old_latitude: i64,
            new_latitude: i64,
            old_longitude: i64,
            new_longitude: i64,
        },
    }

    /// Errors that can be returned by this pallet.
    ///
    /// This type of runtime error can be up to 4 bytes in size should you want to return additional
    /// information.
    #[pallet::error]
    pub enum Error<T> {
        /// Bluetooth Address is already taken
        BluetoothAddressAlreadyTaken,
        /// Account has already registered a node
        AccountAlreadyRegistered,
        /// Account is not registered as a node
        AccountNotRegistered,
        /// Bluetooth Address is not a registered
        BluetoothAddressNotRegistered,
        /// Distance between nodes exceeds maximum allowed distance
        ExceedsMaxDistance,
        /// Node update cooldown period has not elapsed yet
        NodeUpdateCooldownNotElapsed,
    }
}
