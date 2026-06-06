use serde::{Deserialize, Serialize};

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

pub struct OffChainSource {
    config: OffChainConfig,
    #[cfg(feature = "ingestion")]
    client: reqwest::Client,
}

impl OffChainSource {
    pub fn new(config: OffChainConfig) -> Self {
        OffChainSource {
            config,
            #[cfg(feature = "ingestion")]
            client: reqwest::Client::new(),
        }
    }

    pub fn config(&self) -> &OffChainConfig {
        &self.config
    }
}

#[cfg(feature = "ingestion")]
use crate::core::types::TimeSeriesData;

#[cfg(feature = "ingestion")]
impl OffChainSource {
    pub fn with_client(config: OffChainConfig, client: reqwest::Client) -> Self {
        OffChainSource { config, client }
    }

    pub async fn fetch_price_history(
        &self,
        coin_id: &str,
        vs_currency: &str,
        days: u32,
    ) -> anyhow::Result<Vec<TimeSeriesData>> {
        let url = format!(
            "{}/coins/{}/market_chart?vs_currency={}&days={}",
            self.config.api_url, coin_id, vs_currency, days
        );

        let resp: CoinGeckoMarketChart = self.client.get(&url).send().await?.json().await?;

        let asset_id = format!("{}-{}", coin_id, vs_currency).to_uppercase();
        let len = resp.prices.len().min(resp.total_volumes.len());

        let data = (0..len)
            .map(|i| {
                let timestamp = (resp.prices[i][0] / 1000.0) as u64;
                let price = resp.prices[i][1];
                let volume = resp.total_volumes[i][1];

                TimeSeriesData {
                    timestamp,
                    asset_id: asset_id.clone(),
                    price,
                    volume,
                    liquidity: 0.0,
                    tvl: None,
                    borrow_rate: None,
                    utilisation: None,
                }
            })
            .collect();

        Ok(data)
    }
}

#[cfg(feature = "ingestion")]
#[derive(Debug, Deserialize)]
struct CoinGeckoMarketChart {
    prices: Vec<[f64; 2]>,
    total_volumes: Vec<[f64; 2]>,
}
