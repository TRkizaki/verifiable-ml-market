use vml_pipeline::core::*;
use vml_pipeline::provenance::*;

#[test]
fn test_full_pipeline_flow() {
    // 1. Generate features from time-series data
    let engine = FeatureEngine::default();
    let prices = vec![100.0, 102.0, 105.0, 103.0, 108.0, 110.0];

    let rolling = engine.rolling_mean(&prices);
    assert!(rolling[2].is_some());

    let lags = engine.create_lag_features(&prices);
    assert_eq!(lags[0][1], Some(100.0));

    let vol = engine.realised_volatility(&prices);
    assert_eq!(vol.len(), prices.len());

    // 2. Hash input data for provenance
    let input_json = serde_json::to_string(&prices).unwrap();
    let input_hash = DataHasher::hash_string(&input_json);
    assert_eq!(input_hash.len(), 64); // SHA-256 hex

    // 3. Simulate model predictions
    let predictions = ModelPredictions {
        asset_id: "ETH-USD".to_string(),
        timestamp: 1700000000,
        xgboost_prediction: 112.0,
        lstm_prediction: 109.0,
    };

    // 4. Ensemble prediction
    let ensemble = StaticEnsemble::uniform();
    let prediction = ensemble.predict(&predictions);
    assert!((prediction - 110.5).abs() < 0.01);

    // 5. Create commitment
    let model_hash = DataHasher::hash_string("xgboost_v1+lstm_v1");
    let commitment = CommitmentScheme::commit(
        "pred_001",
        prediction,
        &model_hash,
        &input_hash,
        1700000000,
    );

    // 6. Verify commitment
    assert!(CommitmentScheme::verify(
        &commitment.commitment_hash,
        commitment.prediction(),
        commitment.salt(),
        commitment.model_hash(),
        commitment.input_hash(),
    ));

    // 7. Tampered prediction fails verification
    assert!(!CommitmentScheme::verify(
        &commitment.commitment_hash,
        999.0,
        commitment.salt(),
        commitment.model_hash(),
        commitment.input_hash(),
    ));
}

#[test]
fn test_merkle_tree_batch_provenance() {
    let predictions: Vec<String> = vec![
        "pred_001:112.0",
        "pred_002:108.5",
        "pred_003:115.2",
        "pred_004:99.8",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let tree = MerkleTree::from_data(&predictions);

    assert!(!tree.root.is_empty());
    assert_eq!(tree.leaves.len(), 4);
    assert!(tree.verify_leaf(0, "pred_001:112.0"));
    assert!(tree.verify_leaf(3, "pred_004:99.8"));
    assert!(!tree.verify_leaf(0, "pred_001:999.0"));
}

#[test]
fn test_ensemble_optimiser() {
    let optimiser = EnsembleOptimiser::default().with_grid_resolution(20);

    let predictions = vec![
        ModelPredictions {
            asset_id: "ETH".to_string(),
            timestamp: 0,
            xgboost_prediction: 105.0,
            lstm_prediction: 103.0,
        },
        ModelPredictions {
            asset_id: "ETH".to_string(),
            timestamp: 1,
            xgboost_prediction: 110.0,
            lstm_prediction: 112.0,
        },
        ModelPredictions {
            asset_id: "ETH".to_string(),
            timestamp: 2,
            xgboost_prediction: 98.0,
            lstm_prediction: 100.0,
        },
    ];
    let actual = vec![104.0, 111.0, 99.0];

    let weights = optimiser.optimise_grid_search(&predictions, &actual);
    let sum = weights.xgboost_weight + weights.lstm_weight;
    assert!((sum - 1.0).abs() < 0.01);

    let metrics = optimiser.evaluate(&predictions, &actual, &weights);
    assert!(metrics.mae < 2.0);
}
