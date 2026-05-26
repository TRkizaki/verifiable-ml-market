# Verifiable ML Market -- Architecture

## Thesis Contributions

| ID | Contribution | Layer |
|----|-------------|-------|
| C1 | High-performance Rust-Python ML pipeline | Rust Pipeline + Python Models |
| C2 | On-chain verifiable inference protocol (commit-reveal) | Verification Pallet |
| C3 | Tokenised prediction market with Brier-score settlement | Prediction Market Pallet |

## System Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│                     System Architecture                              │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐    ┌──────────────────┐    ┌────────────────┐ │
│  │   Rust Pipeline   │    │  Python Models    │    │ Substrate Node │ │
│  │                   │    │                   │    │                │ │
│  │ - Data Ingestion  │──> │ - XGBoost Model   │──> │ - Verification │ │
│  │ - Feature Eng.    │    │ - LSTM Model      │    │   Pallet       │ │
│  │ - Provenance      │    │ - Model Training  │    │ - Prediction   │ │
│  │   (SHA-256/Merkle)│<── │ - Ensemble Logic  │    │   Market Pallet│ │
│  │ - Commitment      │    │                   │    │ - Balances     │ │
│  │   (Blake2-256)    │    │                   │    │ - Staking      │ │
│  │ - subxt Client    │────────────────────────────>│ - Rewards      │ │
│  └──────────────────┘    └──────────────────┘    └────────────────┘ │
│        Rust                    Python                  Rust          │
│     (Axum HTTP)            (FastAPI/httpx)        (Substrate/FRAME)  │
│     port 3000               port 8000          WS 9944 / HTTP 9933  │
└──────────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
verifiable-ml-market/
├── rust-pipeline/                  # Layer 1: Data pipeline + HTTP API
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── main.rs                 # Axum server entry point
│   │   ├── core/
│   │   │   ├── types.rs            # Domain types (DeFi, time-series)
│   │   │   ├── feature_engine.rs   # Rolling stats, lags, momentum, EMA
│   │   │   └── ensemble.rs         # Weighted average, grid search, gradient descent
│   │   ├── ingestion/
│   │   │   ├── onchain_source.rs   # Substrate RPC reader (subxt)
│   │   │   └── offchain_source.rs  # External API fetcher (CoinGecko)
│   │   ├── provenance/
│   │   │   ├── hasher.rs           # SHA-256, Merkle tree
│   │   │   └── commitment.rs       # Commit-reveal scheme (Blake2-256)
│   │   ├── api/
│   │   │   └── server.rs           # 9 REST endpoints + chain endpoints
│   │   └── substrate_client.rs     # subxt RPC client
│   └── tests/
│       └── integration_tests.rs
│
├── python-models/                  # Layer 2: ML models
│   ├── requirements.txt
│   ├── models/
│   │   ├── xgboost_model.py        # XGBoost: 200 estimators, depth 6
│   │   └── lstm_model.py           # LSTM: PyTorch, [64,32] hidden, dropout 0.2
│   ├── adapters/
│   │   ├── base.py                 # Abstract RustAdapter interface
│   │   └── http_adapter.py         # HTTP client to Rust pipeline
│   └── data/
│
├── substrate-node/                 # Layer 3: Blockchain
│   ├── Cargo.toml                  # Workspace root (polkadot-sdk deps)
│   ├── node/                       # Full Substrate node
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── cli.rs
│   │   │   ├── command.rs
│   │   │   ├── chain_spec.rs       # Dev + local testnet genesis
│   │   │   ├── service.rs          # Aura + GRANDPA consensus
│   │   │   └── rpc.rs
│   │   └── build.rs
│   ├── runtime/                    # FRAME runtime
│   │   └── src/lib.rs              # Wires all pallets into construct_runtime!
│   └── pallets/
│       ├── verification/           # C2: Commit-reveal verification
│       │   └── src/lib.rs
│       └── prediction-market/      # C3: Staking, settlement, rewards
│           └── src/lib.rs
│
├── docker/
│   └── docker-compose.yml          # 3-service orchestration
├── ARCHITECTURE.md
├── PROCEDURE.md                    # Implementation roadmap
└── README.md
```

## On-Chain Protocols

### Commit-Reveal Verification (C2)

The verification pallet implements a commit-reveal scheme that proves an ML prediction
was made before the outcome was known, without revealing it prematurely.

```
                    Model Operator                         Substrate
                         │                                    │
  1. Predict             │                                    │
     value = 1_500_000   │                                    │
     (fixed-point *10^6) │                                    │
                         │                                    │
  2. Commit              │                                    │
     preimage =          │   submit_commitment                │
       prediction (i128 LE bytes)  ───────────────────>       │
       || salt (H256)    │   (prediction_id,                  │ Stores:
       || model_hash     │    commitment_hash)                │   Commitment {
       || input_hash     │                                    │     submitter,
     hash = Blake2_256(preimage)                              │     commitment_hash,
                         │                                    │     revealed: false
                         │                                    │   }
  3. Wait for outcome    │                                    │
                         │                                    │
  4. Reveal              │   reveal_prediction                │
                         │   (prediction_id, prediction,  ──> │ Verifies:
                         │    salt, model_hash, input_hash)   │   Blake2_256(preimage)
                         │                                    │     == stored hash
                         │                                    │ Stores:
                         │                                    │   RevealedPrediction
                         │                                    │
  5. Ground truth        │   submit_ground_truth              │
     (oracle)            │   (prediction_id, outcome)  ────>  │ Stores:
                         │                                    │   GroundTruth
```

### Prediction Market Settlement (C3)

The prediction market uses Brier scoring to reward accurate models
and penalise inaccurate ones.

```
  1. register_model(model_id, model_hash)
       -> ModelInfo { reputation: 5000, ... }

  2. create_market(market_id, prediction_id)
       -> MarketRound { status: Open, ... }

  3. stake_prediction(market_id, model_id, prediction_id, amount)
       -> Currency::reserve(amount)
       -> MarketRound.participant_count++
       -> MarketRound.total_stake += amount

  4. settle_market(market_id)
       -> Read ground truth from verification pallet
       -> For each staked model:
            a. Look up revealed prediction
            b. Compute Brier score:
                 diff = prediction - outcome
                 brier = diff^2 / 10^6
            c. Compute inverse score:
                 inverse = 10^12 - min(brier, 10^12)
       -> Distribute total_stake pool proportional to inverse scores:
            reward_i = total_pool * inverse_i / sum(all_inverse)
       -> Update model reputation:
            if inverse > 5*10^11: correct++, reputation += 100 (cap 10000)
            else: reputation -= 200
       -> Currency::unreserve + deposit rewards
       -> MarketRound.status = Settled
```

### Brier Score Formula

Values use fixed-point representation: `actual_value * 10^6`.

```
brier_score = (prediction - outcome)^2 / 10^6
inverse_score = 10^12 - min(brier_score, 10^12)
reward = total_pool * inverse_score / total_inverse_scores
```

Lower Brier score = more accurate prediction = higher inverse score = larger reward share.

## Communication

```
Python ──HTTP──> Rust Pipeline ──subxt RPC──> Substrate Node
                      │                      (ws://127.0.0.1:9944)
                  Axum Server
                  port 3000
```

### Rust Pipeline <-> Python Models (HTTP/JSON)

| Endpoint                   | Method | Description                    |
|----------------------------|--------|--------------------------------|
| `/health`                  | GET    | Health check                   |
| `/api/features/rolling`    | POST   | Rolling window features        |
| `/api/features/lag`        | POST   | Lag features                   |
| `/api/features/growth`     | POST   | Growth rate features           |
| `/api/ensemble/predict`    | POST   | Ensemble prediction            |
| `/api/ensemble/optimize`   | POST   | Optimise weights               |
| `/api/provenance/hash`     | POST   | SHA-256 hash data              |
| `/api/provenance/commit`   | POST   | Create commitment              |
| `/api/provenance/verify`   | POST   | Verify commitment              |
| `/api/evaluate`            | POST   | Evaluate predictions           |

### Rust Pipeline <-> Substrate (subxt RPC)

| Operation                  | Pallet             | Extrinsic / Storage           |
|----------------------------|--------------------|-------------------------------|
| Submit commitment          | verification       | `submit_commitment`           |
| Reveal prediction          | verification       | `reveal_prediction`           |
| Submit ground truth        | verification       | `submit_ground_truth`         |
| Register model             | prediction_market  | `register_model`              |
| Create market              | prediction_market  | `create_market`               |
| Stake prediction           | prediction_market  | `stake_prediction`            |
| Settle market              | prediction_market  | `settle_market`               |
| Query market state         | prediction_market  | `Markets` storage read        |

## Runtime Configuration

The Substrate runtime (`vml-runtime`) integrates:

| Pallet               | Purpose                                 |
|-----------------------|-----------------------------------------|
| frame_system          | Core runtime types and dispatch         |
| pallet_timestamp      | Block timestamps (Aura slot timing)     |
| pallet_aura           | Block authoring (authority round-robin) |
| pallet_grandpa        | Block finality                          |
| pallet_balances       | Token balances (staking currency)       |
| pallet_transaction_payment | Transaction fees                   |
| pallet_sudo           | Privileged operations (dev/testing)     |
| pallet_verification   | Commit-reveal ML prediction proofs      |
| pallet_prediction_market | Model staking, Brier settlement      |

Cross-pallet dependency: `pallet_prediction_market::Config` requires `pallet_verification::Config`
(reads `GroundTruths` and `Reveals` storage directly).

## Feature Flags (Rust Pipeline)

```toml
[features]
default = ["http"]
http = ["axum", "tokio", ...]      # HTTP API server
python = ["pyo3"]                   # Direct Python bindings (PyO3)
substrate = ["subxt", "sp-core"]    # Substrate RPC client
```

## Docker Services

| Service          | Port  | Description                      |
|------------------|-------|----------------------------------|
| rust-pipeline    | 3000  | Axum HTTP API                    |
| python-models    | 8000  | FastAPI ML serving               |
| substrate-node   | 9944  | Substrate WS RPC (+ 9933 HTTP)  |

## What Was Ported

| Component               | Source (football-rating-predictor)  | Status    |
|--------------------------|------------------------------------|-----------|
| `feature_engine.rs`      | `core/feature_engine.rs`           | Ported    |
| `ensemble.rs`            | `core/ensemble.rs`                 | Ported    |
| `types.rs`               | `core/types.rs`                    | Adapted   |
| `server.rs`              | `api/server.rs`                    | Extended  |
| `Cargo.toml`             | `Cargo.toml`                       | Extended  |
| `adapters/base.py`       | `src/adapters/base.py`             | Adapted   |
| `adapters/http_adapter`  | `src/adapters/http_adapter.py`     | Adapted   |
| `xgboost_model.py`       | `src/models/xgboost_model.py`      | Ported    |
| `lstm_model.py`          | `src/models/lstm_model.py`         | Rewritten |
