//! Benchmarking setup for proof-of-location pallet

use super::*;

#[allow(unused)]
use crate::Pallet as Template;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn set_server_config() {
        let caller: T::AccountId = whitelisted_caller();
        let server_url = b"192.168.1.100:8080".to_vec();

        #[extrinsic_call]
        set_server_config(RawOrigin::Signed(caller.clone()), server_url.clone());

        // Verify the server config was stored
        assert!(ServerConfig::<T>::get(&caller).is_some());
    }

    #[benchmark]
    fn register_node() {
        let caller: T::AccountId = whitelisted_caller();
        let address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let latitude = 37_774_929i64;
        let longitude = -122_419_415i64;

        #[extrinsic_call]
        register_node(
            RawOrigin::Signed(caller.clone()),
            address,
            latitude,
            longitude,
        );

        // Verify the node was registered
        assert!(AccountData::<T>::get(&caller).is_some());
        assert!(AddressRegistrationData::<T>::get(address).is_some());
    }

    #[benchmark]
    fn unregister_node() {
        let caller: T::AccountId = whitelisted_caller();
        let address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let latitude = 37_774_929i64;
        let longitude = -122_419_415i64;

        // Setup: Register the node first
        let _ = Template::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            address,
            latitude,
            longitude,
        );

        #[extrinsic_call]
        unregister_node(RawOrigin::Signed(caller.clone()));

        // Verify the node was unregistered
        assert!(AccountData::<T>::get(&caller).is_none());
        assert!(AddressRegistrationData::<T>::get(address).is_none());
    }

    #[benchmark]
    fn update_node_info() {
        let caller: T::AccountId = whitelisted_caller();
        let old_address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let new_address = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let old_latitude = 37_774_929i64;
        let old_longitude = -122_419_415i64;
        let new_latitude = 40_712_776i64;
        let new_longitude = -74_005_974i64;

        // Setup: Register the node first
        let _ = Template::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            old_address,
            old_latitude,
            old_longitude,
        );

        #[extrinsic_call]
        update_node_info(
            RawOrigin::Signed(caller.clone()),
            new_address,
            new_latitude,
            new_longitude,
        );

        // Verify the node info was updated
        let location_data = AccountData::<T>::get(&caller).unwrap();
        assert_eq!(location_data.address, new_address);
        assert_eq!(location_data.latitude, new_latitude);
        assert_eq!(location_data.longitude, new_longitude);
    }

    #[benchmark]
    fn publish_rssi_data() {
        let caller: T::AccountId = whitelisted_caller();
        let neighbor: T::AccountId = account("neighbor", 0, 0);
        let address1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let address2 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        // Close locations (within MaxDistanceMeters)
        let latitude1 = 37_774_929i64;
        let longitude1 = -122_419_415i64;
        let latitude2 = 37_774_930i64;
        let longitude2 = -122_419_416i64;
        let rssi = -65i16;

        // Setup: Register both nodes
        let _ = Template::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            address1,
            latitude1,
            longitude1,
        );
        let _ = Template::<T>::register_node(
            RawOrigin::Signed(neighbor.clone()).into(),
            address2,
            latitude2,
            longitude2,
        );

        #[extrinsic_call]
        publish_rssi_data(RawOrigin::Signed(caller.clone()), neighbor.clone(), rssi);

        // Verify RSSI data was stored (we can't easily check the exact storage entry
        // without knowing the block number, but the call should succeed)
    }

    impl_benchmark_test_suite!(Template, crate::mock::new_test_ext(), crate::mock::Test);
}
