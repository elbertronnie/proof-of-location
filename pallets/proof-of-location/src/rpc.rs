//! Runtime API definition for trust score calculation

use alloc::vec::Vec;
use codec::Codec;

sp_api::decl_runtime_apis! {
    /// Runtime API for trust score calculations
    pub trait TrustScoreApi<AccountId> where
        AccountId: Codec,
    {
        /// Calculate trust scores for all accounts at a given block number
        ///
        /// # Parameters
        /// - `target_block`: The block number to calculate trust scores for
        ///
        /// # Returns
        /// A vector of trust score data for each account
        fn calculate_trust_scores(target_block: u32) -> Vec<(AccountId, i16)>;

        /// Calculate trust score for a specific account at a given block number
        ///
        /// # Parameters
        /// - `target_block`: The block number to calculate trust score for
        /// - `account`: The account to calculate trust score for
        ///
        /// # Returns
        /// The trust score error value, or None if the account has no data
        fn calculate_trust_score(target_block: u32, account: AccountId) -> Option<i16>;
    }
}
