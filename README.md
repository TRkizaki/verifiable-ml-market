# Verifiable ML Market

High-performance verifiable machine learning for decentralised prediction markets.

A hybrid **Rust-Python** ML pipeline with **Substrate blockchain** for on-chain verification and a tokenised prediction market.

## Architecture

```
Rust Pipeline (data + features + proofs)
    → Python Models (XGBoost + LSTM training/inference)
        → Substrate Node (on-chain verification + prediction market)
```

Three layers, unified by Rust:

| Layer            | Language | Purpose                                    |
|------------------|----------|--------------------------------------------|
| **Data Pipeline** | Rust    | Ingestion, feature engineering, hashing    |
| **ML Models**    | Python   | XGBoost + LSTM training and inference      |
| **Blockchain**   | Rust     | Substrate pallets: verification + market   |

## Quick Start

### Rust Pipeline

```bash
cd rust-pipeline
cargo build --release
cargo run --release
# Server starts on http://localhost:3000
```

### Python Models

```bash
cd python-models
python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

### Run Tests

```bash
# Rust tests
cd rust-pipeline
cargo test

# Integration test (full pipeline flow)
cargo test --test integration_tests
```

## Key Features

- **High-Performance Pipeline**: Rust-based data ingestion and feature engineering with Rayon parallelism
- **Verifiable Inference**: Commit-reveal scheme with SHA-256 — predictions are tamper-proof
- **Data Provenance**: Merkle tree hashing of input data for audit trail
- **Prediction Market**: Substrate pallets for staking, settlement, and reward distribution
- **Domain-Agnostic**: Time-series feature engine works with any sequential data

## Substrate Pallets

### Verification Pallet

Commit-reveal scheme for verifiable ML predictions:

1. `submit_commitment(prediction_id, commitment_hash)` — commit before outcome
2. `reveal_prediction(prediction_id, prediction, salt, model_hash, input_hash)` — reveal after
3. `submit_ground_truth(prediction_id, outcome)` — oracle submits truth

### Prediction Market Pallet

Tokenised competition between ML models:

1. `register_model(model_id, model_hash)` — register a model
2. `stake_prediction(market_id, model_id, prediction_id, amount)` — stake on accuracy
3. `settle_market(market_id)` — distribute rewards based on Brier score

## Tech Stack

- **Rust**: Axum, Serde, Rayon, SHA2, Tokio
- **Python**: PyTorch, XGBoost, FastAPI, httpx
- **Blockchain**: Substrate (Polkadot SDK), FRAME pallets, SCALE codec
- **Integration**: HTTP/JSON (Rust ↔ Python), subxt (Rust ↔ Substrate)

## Thesis

Master's thesis: *"High-Performance Verifiable Machine Learning for Decentralised Prediction Markets: A Hybrid Rust-Python Architecture with Substrate-Based On-Chain Inference"*

## Author

TR Kizaki

## Licence

MIT

