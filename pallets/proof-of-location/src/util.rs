use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;

extern crate alloc;
use alloc::vec::Vec;

#[derive(Encode, Decode, Debug, Clone, TypeInfo)]
pub struct DeviceRssi {
    pub address: [u8; 6],
    pub rssi: i16,
}

#[derive(Encode, Decode, Debug, Clone, TypeInfo)]
pub struct RssiResponse {
    pub devices: Vec<DeviceRssi>,
}

// Using i64 to represent latitude/longitude with fixed-point precision
// Multiply actual coordinates by 1_000_000 to preserve 6 decimal places
#[derive(Encode, Decode, Debug, Clone, TypeInfo, MaxEncodedLen, PartialEq, Eq)]
pub struct LocationData {
    pub address: [u8; 6],
    pub latitude: i64,  // Latitude * 1_000_000
    pub longitude: i64, // Longitude * 1_000_000
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct LocationResponse {
    pub address: [u8; 6],
    pub location: Location,
}

/// Calculate trimmed median error from RSSI values.
/// 
/// Discards the highest 1/4 of values and returns the median of the remaining.
pub fn trimmed_median_error(values: &mut [i16]) -> i16 {
    if values.len() < 4 {
        return i16::MAX;
    }

    // Convert to absolute values
    values.iter_mut().for_each(|x| *x = x.abs());
    values.sort_unstable();

    let len = values.len();
    let trim_end = (len * 3 / 4) as usize;
    let trimmed = &values[..trim_end];

    if trim_end % 2 == 1 {
        trimmed[trim_end / 2]
    } else {
        let mid_upper = trimmed[trim_end / 2];
        let mid_lower = trimmed[trim_end / 2 - 1];
        (mid_upper + mid_lower) / 2
    }
}

/// Estimate RSSI based on distance between two locations.
/// 
/// Uses path loss model: RSSI = r - n * 10 * log10(d).
pub fn estimate_rssi(a_lat: i64, a_lon: i64, b_lat: i64, b_lon: i64) -> i16 {
    // Convert fixed-point coordinates back to f64
    let a_lat_f = a_lat as f64 / 1_000_000.0;
    let a_lon_f = a_lon as f64 / 1_000_000.0;
    let b_lat_f = b_lat as f64 / 1_000_000.0;
    let b_lon_f = b_lon as f64 / 1_000_000.0;

    // Calculate haversine distance using haversine_redux
    use haversine_redux::Location;
    let a = Location::new(a_lat_f, a_lon_f);
    let b = Location::new(b_lat_f, b_lon_f);
    let dist = a.kilometers_to(&b) * 1000.0; // convert km to meters

    // Apply path loss model
    const PATH_LOSS_EXPONENT: f64 = 3.0;
    let rssi = if dist > 0.0 {
        -60.0 - PATH_LOSS_EXPONENT * 10.0 * libm::log10(dist)
    } else {
        0.0
    };
    rssi as i16
}
