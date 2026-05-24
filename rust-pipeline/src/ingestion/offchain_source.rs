use serde::{Deserialize, Serialize};

/// Configuration for off-chain data sources (APIs, price feeds)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffChainConfig {
    pub api_url: String,
    pub api_key: Option<String>,
    pub rate_limit_ms: u64,
}

impl Default for OffChainConfig {
    fn default() -> Self {
        OffChainConfig {
            api_url: "https://api.coingecko.com/api/v3".to_string(),
            api_key: None,
            rate_limit_ms: 1000,
        }
    }
}

/// Off-chain supplementary data source.
/// Fetches market sentiment, cross-chain data, and external price feeds.
pub struct OffChainSource {
    config: OffChainConfig,
}

impl OffChainSource {
    pub fn new(config: OffChainConfig) -> Self {
        OffChainSource { config }
    }

    pub fn config(&self) -> &OffChainConfig {
        &self.config
    }

    // TODO: Implement API-based data fetching
    // pub async fn fetch_price_history(&self, asset: &str, days: u32) -> anyhow::Result<Vec<PricePoint>> {
    //     ...
    // }
}
