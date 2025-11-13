/// RPC implementation functions for the Proof of Location pallet.
///
/// These functions are called by the RPC server to provide external access
/// to pallet functionality without requiring on-chain transactions.

use super::*;
use frame_system::pallet_prelude::BlockNumberFor;

extern crate alloc;
use alloc::vec::Vec;

impl<T: Config> Pallet<T> {
    /// Calculate trust score for a specific account at a given block number.
    /// 
    /// Returns the trimmed median error of RSSI measurements.
    pub fn calculate_trust_score_for_account(
        block_number: BlockNumberFor<T>,
        account: &T::AccountId,
    ) -> Option<i16> {
        use crate::util::{estimate_rssi, trimmed_median_error};

        // Get the location data for the account
        let location_data = AccountData::<T>::get(account)?;

        // Collect all RSSI errors for this account
        let mut errors = Vec::new();

        // Iterate through all possible reporters
        // We need to check RssiData storage for entries with this account as neighbor
        for (reporter_account, reporter_location) in AccountData::<T>::iter() {
            // Skip self
            if reporter_account == *account {
                continue;
            }

            // Check if there's RSSI data from this reporter about our account
            if let Some(measured_rssi) =
                RssiData::<T>::get((block_number, account.clone(), reporter_account.clone()))
            {
                // Calculate estimated RSSI based on location
                let estimated_rssi = estimate_rssi(
                    location_data.latitude,
                    location_data.longitude,
                    reporter_location.latitude,
                    reporter_location.longitude,
                );

                // Calculate error
                let error = measured_rssi - estimated_rssi;
                errors.push(error);
            }
        }

        if errors.is_empty() {
            return None;
        }

        Some(trimmed_median_error(&mut errors))
    }

    /// Calculate trust scores for all accounts at a given block number.
    /// 
    /// Returns a vector of (AccountId, trust_score) tuples.
    pub fn calculate_all_trust_scores(
        block_number: BlockNumberFor<T>,
    ) -> Vec<(T::AccountId, i16)> {
        let mut results = Vec::new();

        for (account, _) in AccountData::<T>::iter() {
            if let Some(score) = Self::calculate_trust_score_for_account(block_number, &account) {
                results.push((account, score));
            }
        }

        results
    }
}
