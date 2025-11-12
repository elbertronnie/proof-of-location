use crate::{mock::*, AccountData, AddressRegistrationData, Error, Event, ServerConfig};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::AccountId32;

// Helper function to create AccountId32 from u32
fn account(id: u32) -> AccountId32 {
    AccountId32::new([id as u8; 32])
}

#[test]
fn set_server_config_works() {
    new_test_ext().execute_with(|| {
        let account = account(1);
        let server_url = b"192.168.1.100:8080".to_vec();

        // Set server configuration
        assert_ok!(ProofOfLocation::set_server_config(
            RuntimeOrigin::signed(account.clone()),
            server_url.clone()
        ));

        // Verify storage was updated
        let stored_config = ServerConfig::<Test>::get(&account).unwrap();
        assert_eq!(stored_config.to_vec(), server_url);
    });
}

#[test]
fn register_node_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let account = account(1);
        let address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let latitude = 37_774_929; // 37.774929 * 1_000_000
        let longitude = -122_419_415; // -122.419415 * 1_000_000

        // Register node
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account.clone()),
            address,
            latitude,
            longitude
        ));

        // Verify storage was updated
        let location_data = AccountData::<Test>::get(&account).unwrap();
        assert_eq!(location_data.address, address);
        assert_eq!(location_data.latitude, latitude);
        assert_eq!(location_data.longitude, longitude);

        // Verify address mapping
        let mapped_account = AddressRegistrationData::<Test>::get(address).unwrap();
        assert_eq!(mapped_account, account);

        // Verify event was emitted
        System::assert_last_event(
            Event::NodeRegistered {
                address,
                who: account.clone(),
                latitude,
                longitude,
            }
            .into(),
        );
    });
}

#[test]
fn register_node_fails_with_duplicate_address() {
    new_test_ext().execute_with(|| {
        let account1 = account(1);
        let account2 = account(2);
        let address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let latitude = 37_774_929;
        let longitude = -122_419_415;

        // First registration succeeds
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account1.clone()),
            address,
            latitude,
            longitude
        ));

        // Second registration with same address fails
        assert_noop!(
            ProofOfLocation::register_node(
                RuntimeOrigin::signed(account2.clone()),
                address,
                latitude,
                longitude
            ),
            Error::<Test>::BluetoothAddressAlreadyTaken
        );
    });
}

#[test]
fn register_node_fails_with_duplicate_account() {
    new_test_ext().execute_with(|| {
        let account = account(1);
        let address1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let address2 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let latitude = 37_774_929;
        let longitude = -122_419_415;

        // First registration succeeds
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account.clone()),
            address1,
            latitude,
            longitude
        ));

        // Second registration with same account fails
        assert_noop!(
            ProofOfLocation::register_node(
                RuntimeOrigin::signed(account.clone()),
                address2,
                latitude,
                longitude
            ),
            Error::<Test>::AccountAlreadyRegistered
        );
    });
}

#[test]
fn unregister_node_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let account = account(1);
        let address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let server_url = b"localhost:3000".to_vec();

        // Register node
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account.clone()),
            address,
            37_774_929,
            -122_419_415
        ));

        // Set server config
        assert_ok!(ProofOfLocation::set_server_config(
            RuntimeOrigin::signed(account.clone()),
            server_url
        ));

        // Unregister node
        assert_ok!(ProofOfLocation::unregister_node(RuntimeOrigin::signed(
            account.clone()
        )));

        // Verify all storage was cleared
        assert_eq!(AccountData::<Test>::get(&account), None);
        assert_eq!(AddressRegistrationData::<Test>::get(address), None);
        assert_eq!(ServerConfig::<Test>::get(&account), None);

        // Verify event was emitted
        System::assert_last_event(
            Event::NodeUnregistered {
                address,
                who: account.clone(),
            }
            .into(),
        );
    });
}

#[test]
fn unregister_node_fails_if_not_registered() {
    new_test_ext().execute_with(|| {
        let account = account(1);

        // Try to unregister without registering first
        assert_noop!(
            ProofOfLocation::unregister_node(RuntimeOrigin::signed(account.clone())),
            Error::<Test>::AccountNotRegistered
        );
    });
}

#[test]
fn update_node_info_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let account = account(1);
        let old_address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let new_address = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let old_latitude = 37_774_929;
        let old_longitude = -122_419_415;
        let new_latitude = 40_712_776;
        let new_longitude = -74_005_974;

        // Register node
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account.clone()),
            old_address,
            old_latitude,
            old_longitude
        ));

        System::set_block_number(2);

        // Update node info
        assert_ok!(ProofOfLocation::update_node_info(
            RuntimeOrigin::signed(account.clone()),
            new_address,
            new_latitude,
            new_longitude
        ));

        // Verify storage was updated
        let location_data = AccountData::<Test>::get(&account).unwrap();
        assert_eq!(location_data.address, new_address);
        assert_eq!(location_data.latitude, new_latitude);
        assert_eq!(location_data.longitude, new_longitude);

        // Verify old address mapping was removed
        assert_eq!(AddressRegistrationData::<Test>::get(old_address), None);

        // Verify new address mapping was created
        let mapped_account = AddressRegistrationData::<Test>::get(new_address).unwrap();
        assert_eq!(mapped_account, account);

        // Verify event was emitted
        System::assert_last_event(
            Event::NodeUpdated {
                who: account.clone(),
                old_address,
                new_address,
                old_latitude,
                new_latitude,
                old_longitude,
                new_longitude,
            }
            .into(),
        );
    });
}

#[test]
fn update_node_info_fails_if_not_registered() {
    new_test_ext().execute_with(|| {
        let account = account(1);
        let address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];

        // Try to update without registering first
        assert_noop!(
            ProofOfLocation::update_node_info(
                RuntimeOrigin::signed(account.clone()),
                address,
                37_774_929,
                -122_419_415
            ),
            Error::<Test>::AccountNotRegistered
        );
    });
}

#[test]
fn update_node_info_fails_if_new_address_taken() {
    new_test_ext().execute_with(|| {
        let account1 = account(1);
        let account2 = account(2);
        let address1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let address2 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];

        // Register both nodes
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account1.clone()),
            address1,
            37_774_929,
            -122_419_415
        ));

        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account2.clone()),
            address2,
            37_774_930,
            -122_419_416
        ));

        // Try to update account1 to use address2 (already taken)
        assert_noop!(
            ProofOfLocation::update_node_info(
                RuntimeOrigin::signed(account1.clone()),
                address2,
                37_774_931,
                -122_419_417
            ),
            Error::<Test>::BluetoothAddressAlreadyTaken
        );
    });
}

#[test]
fn publish_rssi_data_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let account1 = account(1);
        let account2 = account(2);
        let address1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let address2 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        // Close locations (within 10 meters - MaxDistanceMeters)
        let latitude1 = 37_774_929; // 37.774929
        let longitude1 = -122_419_415; // -122.419415
        let latitude2 = 37_774_930; // ~0.11 meters away
        let longitude2 = -122_419_416;
        let rssi = -65i16;

        // Register both nodes
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account1.clone()),
            address1,
            latitude1,
            longitude1
        ));

        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account2.clone()),
            address2,
            latitude2,
            longitude2
        ));

        // Publish RSSI data
        assert_ok!(ProofOfLocation::publish_rssi_data(
            RuntimeOrigin::signed(account1.clone()),
            account2.clone(),
            rssi
        ));

        // Verify event was emitted
        System::assert_last_event(
            Event::RssiStored {
                block_number: 1,
                neighbor: account2,
                who: account1,
                rssi,
            }
            .into(),
        );
    });
}

#[test]
fn publish_rssi_data_fails_if_reporter_not_registered() {
    new_test_ext().execute_with(|| {
        let account1 = account(1);
        let account2 = account(2);
        let address2 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];

        // Only register account2
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account2.clone()),
            address2,
            37_774_929,
            -122_419_415
        ));

        // Try to publish RSSI from unregistered account1
        assert_noop!(
            ProofOfLocation::publish_rssi_data(
                RuntimeOrigin::signed(account1.clone()),
                account2,
                -65
            ),
            Error::<Test>::AccountNotRegistered
        );
    });
}

#[test]
fn publish_rssi_data_fails_if_neighbor_not_registered() {
    new_test_ext().execute_with(|| {
        let account1 = account(1);
        let account2 = account(2);
        let address1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];

        // Only register account1
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account1.clone()),
            address1,
            37_774_929,
            -122_419_415
        ));

        // Try to publish RSSI for unregistered account2
        assert_noop!(
            ProofOfLocation::publish_rssi_data(
                RuntimeOrigin::signed(account1.clone()),
                account2,
                -65
            ),
            Error::<Test>::AccountNotRegistered
        );
    });
}

#[test]
fn publish_rssi_data_fails_if_distance_exceeds_maximum() {
    new_test_ext().execute_with(|| {
        let account1 = account(1);
        let account2 = account(2);
        let address1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let address2 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        // Far apart locations (> 10 meters - MaxDistanceMeters)
        let latitude1 = 37_774_929; // San Francisco
        let longitude1 = -122_419_415;
        let latitude2 = 40_712_776; // New York (very far)
        let longitude2 = -74_005_974;

        // Register both nodes
        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account1.clone()),
            address1,
            latitude1,
            longitude1
        ));

        assert_ok!(ProofOfLocation::register_node(
            RuntimeOrigin::signed(account2.clone()),
            address2,
            latitude2,
            longitude2
        ));

        // Try to publish RSSI data (should fail due to distance)
        assert_noop!(
            ProofOfLocation::publish_rssi_data(
                RuntimeOrigin::signed(account1.clone()),
                account2,
                -65
            ),
            Error::<Test>::ExceedsMaxDistance
        );
    });
}
