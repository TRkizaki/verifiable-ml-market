use serde::{Deserialize, Serialize};
use crate::core::types::TimeSeriesData;

/// Configuration for on-chain data ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainConfig {
    pub rpc_url: String,
    pub start_block: u64,
    pub end_block: Option<u64>,
    pub batch_size: u64,
}

impl Default for OnChainConfig {
    fn default() -> Self {
        OnChainConfig {
            rpc_url: "ws://127.0.0.1:9944".to_string(),
            start_block: 0,
            end_block: None,
            batch_size: 100,
        }
    }
}

/// On-chain data source fetcher.
/// In production, this will use subxt to query Substrate RPC.
/// For now, provides the interface and CSV fallback for development.
pub struct OnChainSource {
    config: OnChainConfig,
}

impl OnChainSource {
    pub fn new(config: OnChainConfig) -> Self {
        OnChainSource { config }
    }

    pub fn config(&self) -> &OnChainConfig {
        &self.config
    }

    /// Load data from CSV file (development fallback)
    pub fn load_from_csv(&self, path: &str) -> anyhow::Result<Vec<TimeSeriesData>> {
        let mut reader = csv::Reader::from_path(path)?;
        let mut data = Vec::new();

        for result in reader.deserialize() {
            let record: TimeSeriesData = result?;
            data.push(record);
        }

        data.sort_by_key(|d| d.timestamp);
        Ok(data)
    }

    // TODO: Implement subxt-based on-chain data fetching
    // #[cfg(feature = "substrate")]
    // pub async fn fetch_blocks(&self, from: u64, to: u64) -> anyhow::Result<Vec<TimeSeriesData>> {
    //     ...
    // }
}
