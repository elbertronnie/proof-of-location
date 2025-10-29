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
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"pof!");

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
    use frame_support::pallet_prelude::*;
    use frame_system::offchain::{
        AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::offchain::{http, Duration};
    use sp_std::prelude::*;

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
        
        /// Default server URL for fetching RSSI data (used if not set via set_server_config)
        #[pallet::constant]
        type ServerUrl: Get<&'static [u8]>;
        
        /// Default server port for fetching RSSI data (used if not set via set_server_config)
        #[pallet::constant]
        type ServerPort: Get<u16>;
    }

    #[derive(Encode, Decode, Debug, Clone, TypeInfo)]
    struct DeviceRssi {
        address: [u8; 6],
        name: Vec<u8>,
        rssi: i16,
    }

    #[derive(Encode, Decode, Debug, Clone, TypeInfo)]
    struct RssiResponse {
        devices: Vec<DeviceRssi>,
    }

    /// A storage item for this pallet.
    ///
    /// In this template, we are declaring a storage item called `Something` that stores a single
    /// `u32` value. Learn more about runtime storage here: <https://docs.substrate.io/build/runtime-storage/>
    #[pallet::storage]
    pub type RssiData<T: Config> = StorageNMap<
        Key = (
            NMapKey<Identity, BlockNumberFor<T>>,
            NMapKey<Blake2_128Concat, [u8; 6]>,
            NMapKey<Blake2_128Concat, T::AccountId>,
        ),
        Value = i16,
        QueryKind = OptionQuery,
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
        /// A user has successfully set a new value.
        RssiStored {
            block_number: BlockNumberFor<T>,
            address: [u8; 6],
            who: T::AccountId,
            rssi: i16,
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
        /// The value retrieved was `None` as no value was previously set.
        NoneValue,
        /// There was an attempt to increment the value in storage over `u32::MAX`.
        StorageOverflow,
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
            address: [u8; 6],
            rssi: i16,
        ) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer.
            let who = ensure_signed(origin)?;

            // Get the current block number.
            let block_number = frame_system::Pallet::<T>::block_number();

            // Update storage.
            RssiData::<T>::insert((block_number, address, who.clone()), rssi);

            // Emit an event.
            Self::deposit_event(Event::RssiStored {
                block_number,
                address,
                who,
                rssi,
            });

            // Return a successful `DispatchResult`
            Ok(())
        }

        /// Set the server configuration for this specific node's offchain worker.
        /// This is stored in offchain local storage and is node-specific.
        ///
        /// This allows each node to connect to a different server without recompiling.
        ///
        /// ## Parameters
        /// - `origin`: Must be root (sudo)
        /// - `server_url`: The server URL (e.g., "localhost", "192.168.1.100")
        /// - `server_port`: The server port (e.g., 3000, 8080)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::cause_error())]
        pub fn set_server_config(
            origin: OriginFor<T>,
            server_url: Vec<u8>,
            server_port: u16,
        ) -> DispatchResult {
            // Only root/sudo can set this
            ensure_root(origin)?;

            // Store in offchain local storage (node-specific)
            let url_key = b"pallet-template::server_url";
            let port_key = b"pallet-template::server_port";

            sp_io::offchain::local_storage_set(
                sp_core::offchain::StorageKind::PERSISTENT,
                url_key,
                &server_url,
            );

            sp_io::offchain::local_storage_set(
                sp_core::offchain::StorageKind::PERSISTENT,
                port_key,
                &server_port.to_le_bytes(),
            );

            log::info!(
                "Server configuration updated: {}:{}",
                core::str::from_utf8(&server_url).unwrap_or("Invalid UTF-8"),
                server_port
            );

            Ok(())
        }

        /// An example dispatchable that may throw a custom error.
        ///
        /// It checks that the caller is a signed origin and reads the current value from the
        /// `Something` storage item. If a current value exists, it is incremented by 1 and then
        /// written back to storage.
        ///
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If no value has been set ([`Error::NoneValue`])
        /// - If incrementing the value in storage causes an arithmetic overflow
        ///   ([`Error::StorageOverflow`])
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::cause_error())]
        pub fn cause_error(origin: OriginFor<T>) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let block_number = frame_system::Pallet::<T>::block_number();

            match RssiData::<T>::get((block_number, [0u8; 6], _who)) {
                Some(old_value) => {
                    let _new_value = old_value
                        .checked_add(1)
                        .ok_or(Error::<T>::StorageOverflow)?;
                    Ok(())
                }
                None => Err(Error::<T>::NoneValue.into()),
            }
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
        /// Fetch RSSI data from the bluetooth server and submit signed transactions
        fn fetch_rssi_and_submit(_block_number: BlockNumberFor<T>) -> Result<(), &'static str> {
            // Fetch RSSI data from the server
            let rssi_response = Self::fetch_rssi_from_server()
                .map_err(|_| "Failed to fetch RSSI data from server")?;

            log::info!(
                "Fetched RSSI data for {} devices",
                rssi_response.devices.len()
            );

            // Get the signer to sign transactions
            let signer = Signer::<T, T::AuthorityId>::all_accounts();
            if !signer.can_sign() {
                log::error!("No local accounts available for signing");
                return Err("No signing keys available");
            }

            // Submit a signed transaction for each device
            for device in rssi_response.devices.iter() {
                log::info!(
                    "Publishing RSSI data - Address: {:?}, RSSI: {}, Name: {:?}",
                    device.address,
                    device.rssi,
                    core::str::from_utf8(&device.name).unwrap_or("Invalid UTF-8")
                );

                // Create the call
                let call = Call::publish_rssi_data {
                    address: device.address,
                    rssi: device.rssi,
                };

                // Send the signed transaction
                let results = signer.send_signed_transaction(|_account| call.clone());

                // Check results
                for (_, result) in &results {
                    match result {
                        Ok(()) => {
                            log::info!(
                                "Successfully submitted transaction for device {:?}",
                                device.address
                            );
                        }
                        Err(e) => {
                            log::error!("Failed to submit transaction: {:?}", e);
                        }
                    }
                }

                if results.is_empty() {
                    log::error!("No transactions were submitted");
                }
            }

            Ok(())
        }

        /// Fetch RSSI data from the bluetooth server
        fn fetch_rssi_from_server() -> Result<RssiResponse, http::Error> {
            // Try to get node-specific configuration from offchain local storage
            let url_key = b"pallet-template::server_url";
            let port_key = b"pallet-template::server_port";

            let server_url_bytes = sp_io::offchain::local_storage_get(
                sp_core::offchain::StorageKind::PERSISTENT,
                url_key,
            );

            let server_port_bytes = sp_io::offchain::local_storage_get(
                sp_core::offchain::StorageKind::PERSISTENT,
                port_key,
            );

            // Build the URL based on configuration
            let url = match (server_url_bytes, server_port_bytes) {
                (Some(url_bytes), Some(port_bytes)) => {
                    let url_str = sp_std::str::from_utf8(&url_bytes)
                        .map_err(|_| http::Error::Unknown)?;
                    let port = u16::from_le_bytes([
                        port_bytes.get(0).copied().unwrap_or(0),
                        port_bytes.get(1).copied().unwrap_or(0),
                    ]);
                    log::info!("Using node-specific server config: {}:{}", url_str, port);
                    alloc::format!("http://{}:{}/rssi", url_str, port)
                }
                _ => {
                    // Fall back to default configuration
                    let default_url = T::ServerUrl::get();
                    let url_str = sp_std::str::from_utf8(default_url)
                        .map_err(|_| http::Error::Unknown)?;
                    let port = T::ServerPort::get();
                    log::info!("Using default server config: {}:{}", url_str, port);
                    alloc::format!("http://{}:{}/rssi", url_str, port)
                }
            };

            log::info!("Fetching RSSI data from: {}", url);

            // Prepare the HTTP request
            let request = http::Request::get(&url);

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
    }
}
