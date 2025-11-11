//! RPC handler for trust score calculation

use std::sync::Arc;

use codec::Codec;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::ErrorObjectOwned,
};
use pallet_template::rpc::TrustScoreApi as TrustScoreRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;

#[rpc(client, server)]
pub trait TrustScoreApi<BlockHash, AccountId> {
    /// Calculate trust scores for all accounts at a given block number
    #[method(name = "trustScore_calculateAll")]
    fn calculate_trust_scores(
        &self,
        block_number: u32,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<(AccountId, i16)>>;

    /// Calculate trust score for a specific account at a given block number
    #[method(name = "trustScore_calculate")]
    fn calculate_trust_score(
        &self,
        block_number: u32,
        account: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<i16>>;
}

/// Trust score RPC handler
pub struct TrustScore<C, Block> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, Block> TrustScore<C, Block> {
    /// Create new instance
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<C, Block, AccountId> TrustScoreApiServer<<Block as BlockT>::Hash, AccountId>
    for TrustScore<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: TrustScoreRuntimeApi<Block, AccountId>,
    AccountId: Codec,
{
    fn calculate_trust_scores(
        &self,
        block_number: u32,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Vec<(AccountId, i16)>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or_else(|| self.client.info().best_hash);

        api.calculate_trust_scores(at, block_number).map_err(|e| {
            ErrorObjectOwned::owned(
                1,
                "Unable to calculate trust scores",
                Some(format!("{:?}", e)),
            )
        })
    }

    fn calculate_trust_score(
        &self,
        block_number: u32,
        account: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Option<i16>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or_else(|| self.client.info().best_hash);

        api.calculate_trust_score(at, block_number, account)
            .map_err(|e| {
                ErrorObjectOwned::owned(
                    1,
                    "Unable to calculate trust score",
                    Some(format!("{:?}", e)),
                )
            })
    }
}
