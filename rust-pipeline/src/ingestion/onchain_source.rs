use crate::core::types::TimeSeriesData;
use serde::{Deserialize, Serialize};

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

pub struct OnChainSource {
    config: OnChainConfig,
    #[cfg(feature = "substrate")]
    client: Option<crate::substrate_client::client::SubstrateClient>,
}

impl OnChainSource {
    pub fn new(config: OnChainConfig) -> Self {
        OnChainSource {
            config,
            #[cfg(feature = "substrate")]
            client: None,
        }
    }

    pub fn config(&self) -> &OnChainConfig {
        &self.config
    }

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
}

#[cfg(feature = "substrate")]
impl OnChainSource {
    pub async fn connect(&mut self) -> anyhow::Result<()> {
        let client =
            crate::substrate_client::client::SubstrateClient::connect(&self.config.rpc_url)
                .await?;
        self.client = Some(client);
        Ok(())
    }

    pub async fn fetch_market_state(
        &self,
        market_id: subxt::utils::H256,
    ) -> anyhow::Result<Option<MarketStateInfo>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to substrate node"))?;

        let market = client.query_market(market_id).await?;
        Ok(market.map(|m| MarketStateInfo {
            status: format!("{:?}", m.status),
            total_stake: m.total_stake,
            participant_count: m.participant_count,
            created_block: m.created_block,
            prediction_id: format!("0x{}", hex::encode(m.prediction_id.0)),
        }))
    }
}

#[cfg(feature = "substrate")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStateInfo {
    pub status: String,
    pub total_stake: u128,
    pub participant_count: u32,
    pub created_block: u64,
    pub prediction_id: String,
}
