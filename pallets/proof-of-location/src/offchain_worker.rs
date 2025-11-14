use frame_support::pallet_macros::*;

/// A [`pallet_section`] that defines the offchain worker for the pallet.
#[pallet_section]
mod offchain {
    use crate::util::{LocationResponse, RssiResponse};

    extern crate alloc;
    use alloc::string::String;
    use alloc::vec::Vec;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Offchain worker entry point.
        ///
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
        fn get_server_base_url() -> Result<String, sp_runtime::offchain::http::Error> {
            use codec::Decode;
            use sp_runtime::offchain::http;

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
        pub fn fetch_rssi_and_submit(_block_number: BlockNumberFor<T>) -> Result<(), &'static str> {
            use codec::{Decode, Encode};
            use frame_system::offchain::{SendSignedTransaction, Signer};

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
        fn fetch_rssi_from_server() -> Result<RssiResponse, sp_runtime::offchain::http::Error> {
            use codec::Decode;
            use sp_runtime::offchain::{http, Duration};

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
        fn fetch_location_from_server(
        ) -> Result<LocationResponse, sp_runtime::offchain::http::Error> {
            use codec::Decode;
            use sp_runtime::offchain::{http, Duration};

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
            use frame_system::offchain::{SendSignedTransaction, Signer};

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
