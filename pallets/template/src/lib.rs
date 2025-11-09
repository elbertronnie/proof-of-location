//! # Template Pallet
//!
//! A pallet with minimal functionality to help developers understand the essential components of
//! writing a FRAME pallet. It is typically used in beginner tutorials or in Substrate template
//! nodes as a starting point for creating a new pallet and **not meant to be used in production**.
//!
//! ## Overview
//!
//! This template pallet contains basic examples of:
//! - declaring a storage item that stores a single `u32` value
//! - declaring and using events
//! - declaring and using errors
//! - a dispatchable function that allows a user to set a new value to storage and emits an event
//!   upon success
//! - another dispatchable function that causes a custom error to be thrown
//!
//! Each pallet section is annotated with an attribute using the `#[pallet::...]` procedural macro.
//! This macro generates the necessary code for a pallet to be aggregated into a FRAME runtime.
//!
//! Learn more about FRAME macros [here](https://docs.substrate.io/reference/frame-macros/).
//!
//! ### Pallet Sections
//!
//! The pallet sections in this template are:
//!
//! - A **configuration trait** that defines the types and parameters which the pallet depends on
//!   (denoted by the `#[pallet::config]` attribute). See: [`Config`].
//! - A **means to store pallet-specific data** (denoted by the `#[pallet::storage]` attribute).
//!   See: [`storage_types`].
//! - A **declaration of the events** this pallet emits (denoted by the `#[pallet::event]`
//!   attribute). See: [`Event`].
//! - A **declaration of the errors** that this pallet can throw (denoted by the `#[pallet::error]`
//!   attribute). See: [`Error`].
//! - A **set of dispatchable functions** that define the pallet's functionality (denoted by the
//!   `#[pallet::call]` attribute). See: [`dispatchables`].
//!
//! Run `cargo doc --package pallet-template --open` to view this pallet's documentation.

// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

// Import alloc for format! macro in no_std
extern crate alloc;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

// // FRAME pallets require their own "mock runtimes" to be able to run unit tests. This module
// // contains a mock runtime specific for testing this pallet's functionality.
// #[cfg(test)]
// mod mock;

// // This module contains the unit tests for this pallet.
// // Learn about pallet unit testing here: https://docs.substrate.io/test/unit-testing/
// #[cfg(test)]
// mod tests;

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

// All pallet logic is defined in its own module and must be annotated by the `pallet` attribute.
#[frame_support::pallet]
pub mod pallet {
    // Import various useful types required by all FRAME pallets.
    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;
    use frame_support::pallet_prelude::*;
    use frame_system::offchain::{
        AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::offchain::{http, Duration};

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

        /// Default server URL with port for fetching data (used if not set via set_server_config)
        /// Format: "hostname:port" or "ip:port" (e.g., "localhost:3000")
        #[pallet::constant]
        type ServerUrl: Get<&'static [u8]>;

        /// Maximum allowed distance between 2 nodes (in meters) to consider publishing RSSI data.
        #[pallet::constant]
        type MaxDistanceMeters: Get<u32>;
    }

    #[derive(Encode, Decode, Debug, Clone, TypeInfo)]
    struct DeviceRssi {
        address: [u8; 6],
        rssi: i16,
    }

    #[derive(Encode, Decode, Debug, Clone, TypeInfo)]
    struct RssiResponse {
        devices: Vec<DeviceRssi>,
    }

    // Using i64 to represent latitude/longitude with fixed-point precision
    // Multiply actual coordinates by 1_000_000 to preserve 6 decimal places
    #[derive(Encode, Decode, Debug, Clone, TypeInfo, MaxEncodedLen, PartialEq, Eq)]
    #[scale_info(skip_type_params(T))]
    pub struct LocationData {
        pub address: [u8; 6],
        pub latitude: i64,  // Latitude * 1_000_000
        pub longitude: i64, // Longitude * 1_000_000
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

    /// A storage item for this pallet.
    ///
    /// In this template, we are declaring a storage item called `Something` that stores a single
    /// `u32` value. Learn more about runtime storage here: <https://docs.substrate.io/build/runtime-storage/>
    #[pallet::storage]
    pub type RssiData<T: Config> = StorageNMap<
        Key = (
            NMapKey<Identity, BlockNumberFor<T>>,
            NMapKey<Blake2_128Concat, T::AccountId>, // neighbor account
            NMapKey<Blake2_128Concat, T::AccountId>, // reporting account
        ),
        Value = i16,
    >;

    #[pallet::storage]
    pub type AddressRegistrationData<T: Config> =
        StorageMap<Hasher = Blake2_128Concat, Key = [u8; 6], Value = T::AccountId>;

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
    /// Events are a simple means of indicating to the outside world (such as dApps, chain explorers
    /// or other users) that some notable update in the runtime has occurred. In a FRAME pallet, the
    /// documentation for each event field and its parameters is added to a node's metadata so it
    /// can be used by external interfaces or tools.
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
    /// Errors tell users that something went wrong so it's important that their naming is
    /// informative. Similar to events, error documentation is added to a node's metadata so it's
    /// equally important that they have helpful documentation associated with them.
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
    }

    /// The pallet's dispatchable functions ([`Call`]s).
    ///
    /// Dispatchable functions allows users to interact with the pallet and invoke state changes.
    /// These functions materialize as "extrinsics", which are often compared to transactions.
    /// They must always return a `DispatchResult` and be annotated with a weight and call index.
    ///
    /// The [`call_index`] macro is used to explicitly
    /// define an index for calls in the [`Call`] enum. This is useful for pallets that may
    /// introduce new dispatchables over time. If the order of a dispatchable changes, its index
    /// will also change which will break backwards compatibility.
    ///
    /// The [`weight`] macro is used to assign a weight to each call.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// An example dispatchable that takes a single u32 value as a parameter, writes the value
        /// to storage and emits an event.
        ///
        /// It checks that the _origin_ for this call is _Signed_ and returns a dispatch
        /// error if it isn't. Learn more about origins here: <https://docs.substrate.io/build/origins/>
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::do_something())]
        pub fn publish_rssi_data(
            origin: OriginFor<T>,
            neighbor: T::AccountId,
            rssi: i16,
        ) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer.
            let who = ensure_signed(origin)?;

            // Check that origin account is registered.
            ensure!(
                AccountData::<T>::contains_key(&who),
                Error::<T>::AccountNotRegistered
            );

            // Check that neighbor account is registered.
            ensure!(
                AccountData::<T>::contains_key(&neighbor),
                Error::<T>::AccountNotRegistered
            );

            // Get account locations
            let reporter_location = AccountData::<T>::get(&who).unwrap();
            let neighbor_location = AccountData::<T>::get(&neighbor).unwrap();

            // Convert them to normal units
            let reporter_latitude = reporter_location.latitude as f64 / 1_000_000.0;
            let reporter_longitude = reporter_location.longitude as f64 / 1_000_000.0;
            let neighbor_latitude = neighbor_location.latitude as f64 / 1_000_000.0;
            let neighbor_longitude = neighbor_location.longitude as f64 / 1_000_000.0;

            use haversine_redux::Location;
            let a = Location::new(reporter_latitude, reporter_longitude);
            let b = Location::new(neighbor_latitude, neighbor_longitude);
            let distance = a.kilometers_to(&b) * 1000.0; // convert km to meters

            // Check that distance is within allowed maximum.
            ensure!(
                distance <= T::MaxDistanceMeters::get() as f64,
                Error::<T>::ExceedsMaxDistance
            );

            // Get the current block number.
            let block_number = frame_system::Pallet::<T>::block_number();

            // Update storage.
            RssiData::<T>::insert((block_number, neighbor.clone(), who.clone()), rssi);

            // Emit an event.
            Self::deposit_event(Event::RssiStored {
                block_number,
                neighbor,
                who,
                rssi,
            });

            // Return a successful `DispatchResult`
            Ok(())
        }

        /// Publish location data to storage.
        ///
        /// This is called by the offchain worker to store location coordinates.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::do_something())]
        pub fn register_node(
            origin: OriginFor<T>,
            address: [u8; 6],
            latitude: i64,
            longitude: i64,
        ) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer.
            let who = ensure_signed(origin)?;

            // Confirm if the bluetooth address is not already taken
            ensure!(
                !AddressRegistrationData::<T>::contains_key(address),
                Error::<T>::BluetoothAddressAlreadyTaken
            );

            // Confirm if the account is not already registered
            ensure!(
                !AccountData::<T>::contains_key(&who),
                Error::<T>::AccountAlreadyRegistered
            );

            // Create location data
            let location_data = LocationData {
                address,
                latitude,
                longitude,
            };

            // Update storage.
            AccountData::<T>::insert(who.clone(), location_data.clone());
            AddressRegistrationData::<T>::insert(address, who.clone());

            // Emit an event.
            Self::deposit_event(Event::NodeRegistered {
                address,
                who,
                latitude,
                longitude,
            });

            // Return a successful `DispatchResult`
            Ok(())
        }

        /// Set the server configuration for a specific account's offchain worker.
        /// This is stored in on-chain storage and is account-specific.
        ///
        /// This allows each node to connect to a different server without recompiling.
        ///
        /// ## Parameters
        /// - `origin`: Must be signed by the account
        /// - `server_url`: The full server URL with port (e.g., "localhost:3000", "192.168.1.100:8080")
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::do_something())]
        pub fn set_server_config(origin: OriginFor<T>, server_url: Vec<u8>) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let who = ensure_signed(origin)?;

            // Convert to BoundedVec
            let bounded_url: BoundedVec<u8, ConstU32<256>> = server_url
                .clone()
                .try_into()
                .map_err(|_| "Server URL too long (max 256 bytes)")?;

            // Store in on-chain storage
            ServerConfig::<T>::insert(who.clone(), bounded_url);

            log::info!(
                "Server configuration updated for account {:?}: {}",
                who,
                core::str::from_utf8(&server_url).unwrap_or("Invalid UTF-8")
            );

            Ok(())
        }

        /// Unregister a node from the network.
        ///
        /// This removes all associated data including location, Bluetooth address mapping,
        /// and server configuration. The caller must be the registered account.
        ///
        /// ## Parameters
        /// - `origin`: Must be signed by the account that registered the node
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::do_something())]
        pub fn unregister_node(origin: OriginFor<T>) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let who = ensure_signed(origin)?;

            // Check that the account is registered
            ensure!(
                AccountData::<T>::contains_key(&who),
                Error::<T>::AccountNotRegistered
            );

            // Get the location data to retrieve the Bluetooth address
            let location_data = AccountData::<T>::get(&who).unwrap();
            let bluetooth_address = location_data.address;

            // Remove from all storage items
            AccountData::<T>::remove(&who);
            AddressRegistrationData::<T>::remove(bluetooth_address);
            ServerConfig::<T>::remove(&who);

            // Emit an event
            Self::deposit_event(Event::NodeUnregistered {
                address: bluetooth_address,
                who,
            });

            log::info!(
                "Node unregistered for account with Bluetooth address {:?}",
                bluetooth_address
            );

            Ok(())
        }

        /// Update node information (location and/or Bluetooth address).
        ///
        /// This allows a registered node to update its location coordinates and/or Bluetooth address.
        /// The node must already be registered.
        ///
        /// ## Parameters
        /// - `origin`: Must be signed by the account that registered the node
        /// - `address`: New Bluetooth address (6 bytes)
        /// - `latitude`: New latitude coordinate (multiply by 1_000_000 for precision)
        /// - `longitude`: New longitude coordinate (multiply by 1_000_000 for precision)
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::do_something())]
        pub fn update_node_info(
            origin: OriginFor<T>,
            address: [u8; 6],
            latitude: i64,
            longitude: i64,
        ) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let who = ensure_signed(origin)?;

            // Check that the account is registered
            ensure!(
                AccountData::<T>::contains_key(&who),
                Error::<T>::AccountNotRegistered
            );

            // Get the current location data to retrieve the old Bluetooth address
            let old_location_data = AccountData::<T>::get(&who).unwrap();
            let old_address = old_location_data.address;

            // If the address is changing, ensure the new address is not already taken
            if old_address != address {
                ensure!(
                    !AddressRegistrationData::<T>::contains_key(address),
                    Error::<T>::BluetoothAddressAlreadyTaken
                );

                // Remove old address mapping and add new one
                AddressRegistrationData::<T>::remove(old_address);
                AddressRegistrationData::<T>::insert(address, who.clone());
            }

            // Create updated location data
            let new_location_data = LocationData {
                address,
                latitude,
                longitude,
            };

            // Update storage
            AccountData::<T>::insert(who.clone(), new_location_data);

            // Emit an event with old and new data
            Self::deposit_event(Event::NodeUpdated {
                who,
                old_address,
                new_address: address,
                old_latitude: old_location_data.latitude,
                new_latitude: latitude,
                old_longitude: old_location_data.longitude,
                new_longitude: longitude,
            });

            log::info!(
                "Node information updated for account with new Bluetooth address {:?}",
                address
            );

            Ok(())
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Offchain worker entry point.
        ///
        /// By implementing `fn offchain_worker` you declare a new offchain worker.
        /// This function will be called when the node is fully synced and a new best block is
        /// successfully imported.
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            log::info!("Offchain worker started at block: {:?}", block_number);

            // Call the function that fetches RSSI data and submits transactions
            if let Err(e) = Self::fetch_rssi_and_submit(block_number) {
                log::error!("Error in offchain worker: {:?}", e);
            }
        }
    }

    impl<T: Config> Pallet<T> {
        /// Get the node identifier (account ID) as a hex string
        /// This retrieves the public key from the keystore
        fn get_node_identifier() -> Result<String, &'static str> {
            // Get the public keys from the keystore using the KEY_TYPE
            let keys = sp_io::crypto::sr25519_public_keys(crate::KEY_TYPE);

            if let Some(key) = keys.first() {
                // Convert the public key to hex string
                let key_bytes: &[u8] = key.as_ref();
                let hex_string = Self::bytes_to_hex(key_bytes);
                Ok(hex_string)
            } else {
                log::warn!("No signing keys available, using default node identifier");
                Ok(alloc::format!("node-unknown"))
            }
        }

        /// Helper function to convert bytes to hex string
        fn bytes_to_hex(bytes: &[u8]) -> String {
            let hex_chars: Vec<u8> = bytes
                .iter()
                .flat_map(|b| {
                    let high = (b >> 4) & 0x0f;
                    let low = b & 0x0f;
                    [Self::hex_char(high), Self::hex_char(low)]
                })
                .collect();
            alloc::format!("0x{}", String::from_utf8_lossy(&hex_chars))
        }

        /// Convert a nibble (4 bits) to its hex character
        fn hex_char(nibble: u8) -> u8 {
            match nibble {
                0..=9 => b'0' + nibble,
                10..=15 => b'a' + (nibble - 10),
                _ => b'?',
            }
        }

        /// Get the server base URL for the current account
        /// Returns the configured URL or falls back to default configuration
        fn get_server_base_url() -> Result<String, http::Error> {
            // Get signing keys to determine account ID
            let keys = sp_io::crypto::sr25519_public_keys(crate::KEY_TYPE);

            if let Some(key) = keys.first() {
                // Convert public key to AccountId
                let account_id = T::AccountId::decode(&mut &key.encode()[..])
                    .map_err(|_| http::Error::Unknown)?;

                // Try to get account-specific configuration from on-chain storage
                if let Some(server_url_bounded) = ServerConfig::<T>::get(&account_id) {
                    let server_url = server_url_bounded.to_vec();
                    let url_str =
                        alloc::str::from_utf8(&server_url).map_err(|_| http::Error::Unknown)?;
                    log::info!("Using account-specific server config: {}", url_str);
                    Ok(alloc::format!("http://{}", url_str))
                } else {
                    // Fall back to default configuration
                    let default_url = T::ServerUrl::get();
                    let url_str =
                        alloc::str::from_utf8(default_url).map_err(|_| http::Error::Unknown)?;
                    log::info!("Using default server config: {}", url_str);
                    Ok(alloc::format!("http://{}", url_str))
                }
            } else {
                log::error!("No signing account available");
                Err(http::Error::Unknown)
            }
        }

        /// Fetch RSSI data from the bluetooth server and submit signed transactions
        fn fetch_rssi_and_submit(_block_number: BlockNumberFor<T>) -> Result<(), &'static str> {
            // Get the signer
            let signer = Signer::<T, T::AuthorityId>::all_accounts();
            if !signer.can_sign() {
                log::error!("No local accounts available for signing");
                return Err("No signing keys available");
            }

            // Get the account ID from the signing key to check registration status
            let keys = sp_io::crypto::sr25519_public_keys(crate::KEY_TYPE);
            let account_id = if let Some(key) = keys.first() {
                T::AccountId::decode(&mut &key.encode()[..])
                    .map_err(|_| "Failed to decode account ID")?
            } else {
                return Err("No signing keys available");
            };

            // Check if this node has already registered by checking AccountData storage
            let is_registered = AccountData::<T>::contains_key(&account_id);

            // If the node is not registered, first register it
            if !is_registered {
                let location_response = Self::fetch_location_from_server()
                    .map_err(|_| "Failed to fetch location data from server")?;

                // Submit location data
                Self::submit_location_data(location_response)?;

                log::info!("Node registration complete");
            }

            // Fetch RSSI data from the server
            let rssi_response = Self::fetch_rssi_from_server()
                .map_err(|_| "Failed to fetch RSSI data from server")?;

            // Submit a signed transaction for each device
            for device in rssi_response.devices.iter() {
                // Map bluetooth address to account
                let account = match AddressRegistrationData::<T>::get(device.address) {
                    Some(account_id) => account_id,
                    None => {
                        log::warn!(
                            "Bluetooth address {:?} not registered, skipping",
                            device.address
                        );
                        continue;
                    }
                };

                let call = Call::publish_rssi_data {
                    neighbor: account,
                    rssi: device.rssi,
                };

                // Send the signed transaction
                let results = signer.send_signed_transaction(|_account| call.clone());

                // Check results
                for (_, result) in &results {
                    if let Err(e) = result {
                        log::error!("Failed to submit RSSI transaction: {:?}", e);
                    }
                }
            }

            Ok(())
        }

        /// Fetch RSSI data from the bluetooth server
        fn fetch_rssi_from_server() -> Result<RssiResponse, http::Error> {
            // Get the server base URL
            let base_url = Self::get_server_base_url()?;
            let url = alloc::format!("{}/rssi", base_url);

            log::info!("Fetching RSSI data from: {}", url);

            // Get node identifier for the header
            let node_id = Self::get_node_identifier().map_err(|_| http::Error::Unknown)?;

            log::info!("Request from node: {}", node_id);

            // Prepare the HTTP request with custom header
            let request = http::Request::get(&url);
            let request = request.add_header("X-Node-ID", &node_id);

            // Set a deadline for the request (30 seconds timeout)
            let timeout = sp_io::offchain::timestamp().add(Duration::from_millis(30_000));

            // Send the request
            let pending = request
                .deadline(timeout)
                .send()
                .map_err(|_| http::Error::IoError)?;

            // Wait for the response
            let response = pending
                .try_wait(timeout)
                .map_err(|_| http::Error::DeadlineReached)?
                .map_err(|_| http::Error::IoError)?;

            // Check the response status
            if response.code != 200 {
                log::error!("HTTP request failed with status code: {}", response.code);
                return Err(http::Error::Unknown);
            }

            // Read the response body
            let body = response.body().collect::<Vec<u8>>();

            // Decode the SCALE-encoded response
            let rssi_response = RssiResponse::decode(&mut &body[..]).map_err(|_| {
                log::error!("Failed to decode RSSI response");
                http::Error::Unknown
            })?;

            Ok(rssi_response)
        }

        /// Fetch location data from the server
        fn fetch_location_from_server() -> Result<LocationResponse, http::Error> {
            // Get the server base URL
            let base_url = Self::get_server_base_url()?;
            let url = alloc::format!("{}/location", base_url);

            log::info!("Fetching location data from: {}", url);

            // Get node identifier for the header
            let node_id = Self::get_node_identifier().map_err(|_| http::Error::Unknown)?;

            log::info!("Request from node: {}", node_id);

            // Prepare the HTTP request with custom header
            let request = http::Request::get(&url);
            let request = request.add_header("X-Node-ID", &node_id);

            // Set a deadline for the request (30 seconds timeout)
            let timeout = sp_io::offchain::timestamp().add(Duration::from_millis(30_000));

            // Send the request
            let pending = request
                .deadline(timeout)
                .send()
                .map_err(|_| http::Error::IoError)?;

            // Wait for the response
            let response = pending
                .try_wait(timeout)
                .map_err(|_| http::Error::DeadlineReached)?
                .map_err(|_| http::Error::IoError)?;

            // Check the response status
            if response.code != 200 {
                log::error!("HTTP request failed with status code: {}", response.code);
                return Err(http::Error::Unknown);
            }

            // Read the response body
            let body = response.body().collect::<Vec<u8>>();

            // Decode the SCALE-encoded response
            let location_response = LocationResponse::decode(&mut &body[..]).map_err(|_| {
                log::error!("Failed to decode location response");
                http::Error::Unknown
            })?;

            Ok(location_response)
        }

        /// Submit location data as a signed transaction
        fn submit_location_data(location_data: LocationResponse) -> Result<(), &'static str> {
            // Convert f64 to i64 with fixed-point precision (multiply by 1_000_000)
            let latitude_fixed = (location_data.location.latitude * 1_000_000.0) as i64;
            let longitude_fixed = (location_data.location.longitude * 1_000_000.0) as i64;

            // Create the call
            let call = Call::register_node {
                address: location_data.address,
                latitude: latitude_fixed,
                longitude: longitude_fixed,
            };

            // Get signer and send the transaction
            let signer = Signer::<T, T::AuthorityId>::any_account();
            let result = signer.send_signed_transaction(|_account| call.clone());

            // Check result
            match result {
                Some((_, Ok(()))) => {
                    log::info!("Successfully submitted location data transaction");
                    Ok(())
                }
                Some((_account, Err(e))) => {
                    log::error!("Failed to submit location transaction: {:?}", e);
                    Err("Transaction submission failed")
                }
                None => {
                    log::error!("No signing account available");
                    Err("No signing account available")
                }
            }
        }
    }
}
