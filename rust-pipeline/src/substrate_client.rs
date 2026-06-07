#[cfg(feature = "substrate")]
pub mod client {
    use anyhow::Result;
    use blake2::Digest;
    use subxt::utils::H256;
    use subxt::{OnlineClient, SubstrateConfig};
    use subxt_signer::sr25519::Keypair;

    #[subxt::subxt(runtime_metadata_path = "metadata/vml_runtime.scale")]
    pub mod vml_runtime {}

    pub use vml_runtime::runtime_types::pallet_prediction_market::pallet::MarketRound;

    pub struct SubstrateClient {
        api: OnlineClient<SubstrateConfig>,
        signer: Keypair,
    }

    impl SubstrateClient {
        pub async fn connect(url: &str) -> Result<Self> {
            let api = OnlineClient::<SubstrateConfig>::from_url(url).await?;
            let signer = subxt_signer::sr25519::dev::alice();
            Ok(Self { api, signer })
        }

        pub async fn connect_with_signer(url: &str, signer: Keypair) -> Result<Self> {
            let api = OnlineClient::<SubstrateConfig>::from_url(url).await?;
            Ok(Self { api, signer })
        }

        pub async fn submit_commitment(
            &self,
            prediction_id: H256,
            commitment_hash: H256,
        ) -> Result<H256> {
            let tx = vml_runtime::tx()
                .verification()
                .submit_commitment(prediction_id, commitment_hash);
            self.submit(tx).await
        }

        pub async fn reveal_prediction(
            &self,
            prediction_id: H256,
            prediction: i128,
            salt: H256,
            model_hash: H256,
            input_hash: H256,
        ) -> Result<H256> {
            let tx = vml_runtime::tx()
                .verification()
                .reveal_prediction(prediction_id, prediction, salt, model_hash, input_hash);
            self.submit(tx).await
        }

        pub async fn submit_ground_truth(
            &self,
            prediction_id: H256,
            outcome: i128,
        ) -> Result<H256> {
            let tx = vml_runtime::tx()
                .verification()
                .submit_ground_truth(prediction_id, outcome);
            self.submit(tx).await
        }

        pub async fn register_model(
            &self,
            model_id: H256,
            model_hash: H256,
        ) -> Result<H256> {
            let tx = vml_runtime::tx()
                .prediction_market()
                .register_model(model_id, model_hash);
            self.submit(tx).await
        }

        pub async fn create_market(
            &self,
            market_id: H256,
            prediction_id: H256,
        ) -> Result<H256> {
            let tx = vml_runtime::tx()
                .prediction_market()
                .create_market(market_id, prediction_id);
            self.submit(tx).await
        }

        pub async fn stake_prediction(
            &self,
            market_id: H256,
            model_id: H256,
            prediction_id: H256,
            stake_amount: u128,
        ) -> Result<H256> {
            let tx = vml_runtime::tx()
                .prediction_market()
                .stake_prediction(market_id, model_id, prediction_id, stake_amount);
            self.submit(tx).await
        }

        pub async fn settle_market(&self, market_id: H256) -> Result<H256> {
            let tx = vml_runtime::tx()
                .prediction_market()
                .settle_market(market_id);
            self.submit(tx).await
        }

        pub async fn query_market(&self, market_id: H256) -> Result<Option<MarketRound>> {
            let storage_query = vml_runtime::storage()
                .prediction_market()
                .markets(market_id);
            let result = self
                .api
                .storage()
                .at_latest()
                .await?
                .fetch(&storage_query)
                .await?;
            Ok(result)
        }

        async fn submit<T: subxt::tx::Payload>(&self, tx: T) -> Result<H256> {
            let progress = self
                .api
                .tx()
                .sign_and_submit_then_watch_default(&tx, &self.signer)
                .await?;
            let in_block = progress.wait_for_finalized().await?;
            Ok(in_block.block_hash())
        }
    }

    /// Compute blake2-256 commitment hash matching the on-chain pallet format.
    /// preimage = prediction(i128 LE) || salt(32 bytes) || model_hash(32 bytes) || input_hash(32 bytes)
    pub fn compute_commitment_hash(
        prediction: i128,
        salt: H256,
        model_hash: H256,
        input_hash: H256,
    ) -> H256 {
        let mut preimage = Vec::with_capacity(128);
        preimage.extend_from_slice(&prediction.to_le_bytes());
        preimage.extend_from_slice(salt.as_bytes());
        preimage.extend_from_slice(model_hash.as_bytes());
        preimage.extend_from_slice(input_hash.as_bytes());

        let hash = blake2::Blake2b::<blake2::digest::typenum::U32>::digest(&preimage);
        H256::from_slice(&hash)
    }

    pub fn parse_h256(s: &str) -> Result<H256> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s)?;
        anyhow::ensure!(bytes.len() == 32, "H256 must be 32 bytes, got {}", bytes.len());
        Ok(H256::from_slice(&bytes))
    }

    pub fn format_h256(h: &H256) -> String {
        format!("0x{}", hex::encode(h.0))
    }
}
