# Verifiable ML Market — Architecture

## System Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│                     System Architecture                              │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐    ┌──────────────────┐    ┌────────────────┐ │
│  │   Rust Pipeline   │    │  Python Models    │    │ Substrate Node │ │
│  │                   │    │                   │    │                │ │
│  │ • Data Ingestion  │──► │ • XGBoost Model   │──► │ • Verification │ │
│  │ • Feature Eng.    │    │ • LSTM Model      │    │   Pallet       │ │
│  │ • Proof Generation│    │ • Model Training  │    │ • Prediction   │ │
│  │ • Data Provenance │◄── │ • Ensemble Logic  │    │   Market Pallet│ │
│  │   (SHA-256)       │    │                   │    │ • Staking &    │ │
│  │                   │    │                   │    │   Rewards      │ │
│  └──────────────────┘    └──────────────────┘    └────────────────┘ │
│        Rust                    Python                  Rust          │
│     (Axum HTTP)            (FastAPI/httpx)        (Substrate/FRAME)  │
└──────────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
verifiable-ml-market/
├── rust-pipeline/                  # Rust data pipeline + HTTP API
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── main.rs                 # Axum HTTP server entry point
│   │   ├── core/                   # Domain-agnostic core logic
│   │   │   ├── types.rs            # Data types (ported + extended)
│   │   │   ├── feature_engine.rs   # Time-series features (ported)
│   │   │   └── ensemble.rs         # Ensemble optimisation (ported)
│   │   ├── ingestion/              # Data sources [NEW]
│   │   │   ├── onchain_source.rs   # Substrate RPC data fetcher
│   │   │   └── offchain_source.rs  # External API data fetcher
│   │   ├── provenance/             # Cryptographic verification [NEW]
│   │   │   ├── hasher.rs           # SHA-256, Merkle tree
│   │   │   └── commitment.rs       # Commit-reveal scheme
│   │   ├── api/                    # HTTP API (ported + extended)
│   │   │   └── server.rs           # Routes + handlers
│   │   └── substrate_client.rs     # subxt RPC client [NEW]
│   └── tests/
│       └── integration_tests.rs
│
├── python-models/                  # Python ML models
│   ├── requirements.txt
│   ├── models/
│   │   ├── xgboost_model.py        # XGBoost predictor (ported)
│   │   └── lstm_model.py           # LSTM predictor (ported, PyTorch)
│   ├── adapters/                   # Rust communication layer (ported)
│   │   ├── base.py                 # Abstract interface
│   │   └── http_adapter.py         # HTTP implementation
│   └── data/
│
├── substrate-node/                 # Substrate blockchain [NEW]
│   └── pallets/
│       ├── verification/           # Commit-reveal verification
│       │   └── src/lib.rs
│       └── prediction-market/      # Staking, settlement, rewards
│           └── src/lib.rs
│
├── docker/
│   └── docker-compose.yml
├── .gitignore
├── ARCHITECTURE.md
└── README.md
```

## Communication

```
Python ──HTTP──► Rust Pipeline ──subxt RPC──► Substrate Node
                      │
                  Axum Server
                  port 3000
```

### Rust Pipeline ↔ Python Models

HTTP/JSON API (same pattern as football-rating-predictor):

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

### Rust Pipeline ↔ Substrate

Via `subxt` crate (Rust-native Substrate RPC client):
- Submit commitments to Verification Pallet
- Reveal predictions after ground truth
- Stake predictions in Market Pallet
- Query market state and results

## Feature Flags

```toml
[features]
default = ["http"]
http = ["axum", "tokio", ...]      # HTTP API server
python = ["pyo3"]                   # Direct Python bindings
substrate = ["subxt", "sp-core"]    # Substrate RPC client
```

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
