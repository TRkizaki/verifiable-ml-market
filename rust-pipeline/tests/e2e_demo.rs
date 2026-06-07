//! End-to-end integration test demonstrating all three thesis contributions:
//! C1: High-performance Rust-Python ML pipeline (feature engineering + ensemble)
//! C2: On-chain verifiable inference (commit-reveal via substrate)
//! C3: Tokenised prediction market with Brier-score settlement
//!
//! Requires a running substrate node: ./target/release/vml-node --dev

#[cfg(feature = "substrate")]
mod e2e {
    use std::time::Instant;
    use vml_pipeline::core::*;
    use vml_pipeline::provenance::*;
    use vml_pipeline::substrate_client::client::*;

    const SCALE: i128 = 1_000_000; // fixed-point scale: 10^6

    fn to_fixed(val: f64) -> i128 {
        (val * SCALE as f64) as i128
    }

    fn make_h256(seed: u8) -> subxt::utils::H256 {
        let mut bytes = [0u8; 32];
        bytes[31] = seed;
        subxt::utils::H256(bytes)
    }

    #[tokio::test]
    #[ignore]
    async fn test_full_e2e_pipeline() {
        println!("\n=== Verifiable ML Prediction Market — End-to-End Demo ===\n");

        // ---------------------------------------------------------------
        // C1: Feature Engineering & Ensemble Prediction
        // ---------------------------------------------------------------
        println!("--- C1: High-Performance ML Pipeline ---\n");

        let bench_start = Instant::now();

        let engine = FeatureEngine::default();
        let n = 100;
        let timestamps: Vec<u64> = (0..n).map(|i| 1700000000 + i * 3600).collect();
        let prices: Vec<f64> = (0..n)
            .map(|i| 40000.0 + (i as f64) * 50.0 + ((i as f64) * 0.5).sin() * 200.0)
            .collect();
        let volumes: Vec<f64> = (0..n).map(|i| 1e9 + (i as f64) * 1e7).collect();
        let liquidities: Vec<f64> = vec![5e8; n as usize];
        let tvls: Vec<Option<f64>> = vec![Some(2e10); n as usize];
        let borrow_rates: Vec<Option<f64>> = vec![Some(0.05); n as usize];
        let utilisations: Vec<Option<f64>> = vec![Some(0.75); n as usize];

        let features = engine.generate_features(
            "ETH-USD",
            &timestamps,
            &prices,
            &volumes,
            &liquidities,
            &tvls,
            &borrow_rates,
            &utilisations,
            AssetClass::DeFiToken,
        );

        let feature_gen_ms = bench_start.elapsed().as_micros() as f64 / 1000.0;
        println!(
            "  Feature generation: {} vectors from {} data points in {:.2} ms",
            features.len(),
            n,
            feature_gen_ms
        );
        assert_eq!(features.len(), n as usize);

        let last_feature = &features[features.len() - 1];
        let predictions = ModelPredictions {
            asset_id: "ETH-USD".to_string(),
            timestamp: last_feature.timestamp,
            xgboost_prediction: last_feature.price * 1.002,
            lstm_prediction: last_feature.price * 1.005,
        };

        let ensemble = StaticEnsemble::uniform();
        let predicted_value = ensemble.predict(&predictions);
        println!(
            "  Ensemble prediction: {:.2} (XGBoost: {:.2}, LSTM: {:.2})",
            predicted_value, predictions.xgboost_prediction, predictions.lstm_prediction
        );

        let input_json = serde_json::to_string(&features).unwrap();
        let input_hash_str = DataHasher::hash_string(&input_json);
        let model_hash_str = DataHasher::hash_string("xgboost_v1+lstm_v1");

        let commit_start = Instant::now();
        let commitment = CommitmentScheme::commit(
            "pred_e2e_001",
            predicted_value,
            &model_hash_str,
            &input_hash_str,
            last_feature.timestamp,
        );
        let commit_ms = commit_start.elapsed().as_micros() as f64 / 1000.0;
        println!("  Commitment generated in {:.3} ms", commit_ms);

        assert!(CommitmentScheme::verify(
            &commitment.commitment_hash,
            predicted_value,
            commitment.salt(),
            commitment.model_hash(),
            commitment.input_hash(),
        ));
        println!("  Off-chain commitment verification: PASS");

        // ---------------------------------------------------------------
        // C2: On-Chain Verifiable Inference (Commit-Reveal)
        // ---------------------------------------------------------------
        println!("\n--- C2: On-Chain Verifiable Inference ---\n");

        let client = SubstrateClient::connect("ws://127.0.0.1:9944")
            .await
            .expect("Failed to connect to substrate node — is vml-node --dev running?");
        println!("  Connected to substrate node");

        let prediction_id = make_h256(1);
        let model_id = make_h256(2);
        let market_id = make_h256(3);
        let salt = make_h256(42);
        let model_hash = make_h256(100);
        let input_hash = make_h256(200);
        let prediction_fixed = to_fixed(predicted_value);
        let ground_truth_fixed = to_fixed(predicted_value * 0.998);

        let commitment_hash =
            compute_commitment_hash(prediction_fixed, salt, model_hash, input_hash);

        let tx_hash = client
            .submit_commitment(prediction_id, commitment_hash)
            .await
            .expect("submit_commitment failed");
        println!("  submit_commitment tx: {}", format_h256(&tx_hash));

        // ---------------------------------------------------------------
        // C3: Prediction Market (Register, Market, Stake, Settle)
        // ---------------------------------------------------------------
        println!("\n--- C3: Tokenised Prediction Market ---\n");

        let tx_hash = client
            .register_model(model_id, model_hash)
            .await
            .expect("register_model failed");
        println!("  register_model tx: {}", format_h256(&tx_hash));

        let tx_hash = client
            .create_market(market_id, prediction_id)
            .await
            .expect("create_market failed");
        println!("  create_market tx: {}", format_h256(&tx_hash));

        let stake_amount: u128 = 1_000_000_000_000;
        let tx_hash = client
            .stake_prediction(market_id, model_id, prediction_id, stake_amount)
            .await
            .expect("stake_prediction failed");
        println!(
            "  stake_prediction tx: {} (stake: {})",
            format_h256(&tx_hash),
            stake_amount
        );

        let tx_hash = client
            .submit_ground_truth(prediction_id, ground_truth_fixed)
            .await
            .expect("submit_ground_truth failed");
        println!("  submit_ground_truth tx: {}", format_h256(&tx_hash));

        let tx_hash = client
            .reveal_prediction(prediction_id, prediction_fixed, salt, model_hash, input_hash)
            .await
            .expect("reveal_prediction failed");
        println!("  reveal_prediction tx: {} (hash verified on-chain!)", format_h256(&tx_hash));

        let tx_hash = client
            .settle_market(market_id)
            .await
            .expect("settle_market failed");
        println!("  settle_market tx: {}", format_h256(&tx_hash));

        let market = client
            .query_market(market_id)
            .await
            .expect("query_market failed");
        assert!(market.is_some(), "Market should exist");
        let market = market.unwrap();
        println!(
            "\n  Market state: status={:?}, participants={}, total_stake={}",
            market.status, market.participant_count, market.total_stake
        );

        // ---------------------------------------------------------------
        // Performance Summary
        // ---------------------------------------------------------------
        println!("\n--- Performance Benchmarks ---\n");
        println!(
            "  Feature generation throughput: {:.0} vectors/ms ({} vectors in {:.2} ms)",
            features.len() as f64 / feature_gen_ms,
            features.len(),
            feature_gen_ms
        );
        println!("  Commitment generation: {:.3} ms", commit_ms);
        println!("  Fixed-point scale: 10^6");
        println!(
            "  Prediction (fixed): {} (original: {:.2})",
            prediction_fixed, predicted_value
        );
        println!(
            "  Ground truth (fixed): {} (original: {:.2})",
            ground_truth_fixed,
            predicted_value * 0.998
        );

        let diff = (prediction_fixed - ground_truth_fixed).abs();
        let brier = diff.saturating_mul(diff) / SCALE;
        let inverse = (SCALE * SCALE) - brier.min(SCALE * SCALE);
        println!("  Brier score: {}", brier);
        println!("  Inverse score: {}", inverse);

        println!("\n=== E2E Demo Complete — All Three Contributions Demonstrated ===\n");
    }
}
