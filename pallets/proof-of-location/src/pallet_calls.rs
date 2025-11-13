use frame_support::pallet_macros::*;

/// A [`pallet_section`] that defines the dispatchable calls for the pallet.
#[pallet_section]
mod dispatches {
    /// The pallet's dispatchable functions ([`Call`]s).
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set the server configuration for a specific account's offchain worker.
        /// This is stored in on-chain storage and is account-specific.
        ///
        /// This allows each node to connect to a different server without recompiling.
        ///
        /// ## Parameters
        /// - `origin`: Must be signed by the account
        /// - `server_url`: The full server URL with port (e.g., "localhost:3000", "192.168.1.100:8080")
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_server_config())]
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

        /// Publish location data to storage.
        ///
        /// This is called by the offchain worker to store location coordinates.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::register_node())]
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

        /// Unregister a node from the network.
        ///
        /// This removes all associated data including location, Bluetooth address mapping,
        /// and server configuration. The caller must be the registered account.
        ///
        /// ## Parameters
        /// - `origin`: Must be signed by the account that registered the node
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::unregister_node())]
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
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::update_node_info())]
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

        /// Publish RSSI (signal strength) data for a neighboring node.
        ///
        /// This function stores RSSI measurements between nodes, validating that:
        /// - Both the reporting node and neighbor are registered
        /// - The distance between nodes is within the configured maximum
        ///
        /// ## Parameters
        /// - `origin`: Must be signed by the reporting node's account
        /// - `neighbor`: The AccountId of the neighboring node being measured
        /// - `rssi`: The signal strength measurement (i16, typically negative dBm values)
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::publish_rssi_data())]
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
    }
}
