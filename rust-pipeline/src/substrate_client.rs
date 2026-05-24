/// Substrate RPC client for interacting with the prediction market chain.
/// Uses subxt to submit commitments, reveal predictions, and query market state.
///
/// This module is gated behind the `substrate` feature flag.
/// Enable with: cargo build --features substrate

#[cfg(feature = "substrate")]
pub mod client {
    // TODO: Generate subxt metadata from running substrate-node
    // #[subxt::subxt(runtime_metadata_path = "../substrate-node/metadata.scale")]
    // pub mod prediction_market {}

    pub struct SubstrateClient {
        pub rpc_url: String,
    }

    impl SubstrateClient {
        pub fn new(rpc_url: &str) -> Self {
            SubstrateClient {
                rpc_url: rpc_url.to_string(),
            }
        }

        // TODO: Implement after substrate-node pallets are built
        // pub async fn submit_commitment(&self, commitment_hash: &str) -> Result<()> { ... }
        // pub async fn reveal_prediction(&self, prediction: f64, salt: &str, ...) -> Result<()> { ... }
        // pub async fn stake_prediction(&self, prediction_id: &str, amount: u128) -> Result<()> { ... }
        // pub async fn query_market_state(&self, market_id: &str) -> Result<MarketState> { ... }
    }
}
