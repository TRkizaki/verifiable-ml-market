use axum::{
    extract::State,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::core::*;
use crate::provenance::*;

pub struct AppState {
    pub feature_engine: FeatureEngine,
    pub ensemble_optimiser: EnsembleOptimiser,
    #[cfg(feature = "substrate")]
    pub chain_client: Option<crate::substrate_client::client::SubstrateClient>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            feature_engine: FeatureEngine::default(),
            ensemble_optimiser: EnsembleOptimiser::default(),
            #[cfg(feature = "substrate")]
            chain_client: None,
        }
    }
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        .route("/health", get(health_check))
        // Feature engineering
        .route("/api/features/rolling", post(compute_rolling_features))
        .route("/api/features/lag", post(compute_lag_features))
        .route("/api/features/growth", post(compute_growth_rate))
        // Ensemble
        .route("/api/ensemble/predict", post(ensemble_predict))
        .route("/api/ensemble/optimize", post(optimize_ensemble))
        // Provenance
        .route("/api/provenance/hash", post(hash_data))
        .route("/api/provenance/commit", post(create_commitment))
        .route("/api/provenance/verify", post(verify_commitment))
        // Evaluation
        .route("/api/evaluate", post(evaluate_predictions));

    #[cfg(feature = "ingestion")]
    let router = router
        .route("/api/ingest/prices", post(ingest_prices));

    #[cfg(feature = "substrate")]
    let router = router
        .route("/api/chain/register-model", post(chain_register_model))
        .route("/api/chain/create-market", post(chain_create_market))
        .route("/api/chain/submit-commitment", post(chain_submit_commitment))
        .route("/api/chain/reveal", post(chain_reveal))
        .route("/api/chain/ground-truth", post(chain_ground_truth))
        .route("/api/chain/stake", post(chain_stake))
        .route("/api/chain/settle", post(chain_settle))
        .route("/api/chain/market/{id}", get(chain_query_market));

    router.layer(cors).with_state(state)
}

pub async fn start_server(host: &str, port: u16, #[cfg(feature = "substrate")] substrate_url: Option<&str>) -> anyhow::Result<()> {
    #[allow(unused_mut)]
    let mut state = AppState::default();

    #[cfg(feature = "substrate")]
    if let Some(url) = substrate_url {
        match crate::substrate_client::client::SubstrateClient::connect(url).await {
            Ok(client) => {
                info!("Connected to substrate node at {}", url);
                state.chain_client = Some(client);
            }
            Err(e) => {
                tracing::warn!("Failed to connect to substrate node at {}: {}", url, e);
            }
        }
    }

    let state = Arc::new(state);
    let app = create_router(state);

    let addr = format!("{}:{}", host, port);
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// --- Request/Response types ---

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct RollingFeaturesRequest {
    pub values: Vec<f64>,
    pub window_size: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct RollingFeaturesResponse {
    pub rolling_mean: Vec<Option<f64>>,
    pub rolling_std: Vec<Option<f64>>,
    pub rolling_min: Vec<Option<f64>>,
    pub rolling_max: Vec<Option<f64>>,
}

#[derive(Debug, Deserialize)]
pub struct LagFeaturesRequest {
    pub values: Vec<f64>,
    pub lags: Vec<usize>,
}

#[derive(Debug, Serialize)]
pub struct LagFeaturesResponse {
    pub lag_features: Vec<Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
pub struct GrowthRateRequest {
    pub values: Vec<f64>,
    pub periods: usize,
}

#[derive(Debug, Serialize)]
pub struct GrowthRateResponse {
    pub growth_rates: Vec<Option<f64>>,
}

#[derive(Debug, Deserialize)]
pub struct EnsemblePredictRequest {
    pub predictions: ModelPredictions,
    pub weights: Option<EnsembleWeights>,
}

#[derive(Debug, Serialize)]
pub struct EnsemblePredictResponse {
    pub final_prediction: f64,
    pub weights_used: EnsembleWeights,
}

#[derive(Debug, Deserialize)]
pub struct OptimizeEnsembleRequest {
    pub predictions: Vec<ModelPredictions>,
    pub actual: Vec<f64>,
    pub method: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OptimizeEnsembleResponse {
    pub optimized_weights: EnsembleWeights,
    pub metrics: EvaluationMetrics,
}

#[derive(Debug, Deserialize)]
pub struct HashDataRequest {
    pub data: String,
}

#[derive(Debug, Serialize)]
pub struct HashDataResponse {
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommitmentRequest {
    pub prediction_id: String,
    pub prediction: f64,
    pub model_hash: String,
    pub input_hash: String,
    pub timestamp: u64,
}

#[derive(Debug, Serialize)]
pub struct CreateCommitmentResponse {
    pub commitment_hash: String,
    pub salt: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyCommitmentRequest {
    pub commitment_hash: String,
    pub prediction: f64,
    pub salt: String,
    pub model_hash: String,
    pub input_hash: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyCommitmentResponse {
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
pub struct EvaluateRequest {
    pub predictions: Vec<f64>,
    pub actual: Vec<f64>,
}

// --- Handlers ---

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn compute_rolling_features(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<RollingFeaturesRequest>,
) -> Json<RollingFeaturesResponse> {
    let engine = if let Some(window) = request.window_size {
        FeatureEngine::new(window, vec![1, 2, 3])
    } else {
        FeatureEngine::default()
    };

    Json(RollingFeaturesResponse {
        rolling_mean: engine.rolling_mean(&request.values),
        rolling_std: engine.rolling_std(&request.values),
        rolling_min: engine.rolling_min(&request.values),
        rolling_max: engine.rolling_max(&request.values),
    })
}

async fn compute_lag_features(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<LagFeaturesRequest>,
) -> Json<LagFeaturesResponse> {
    let engine = FeatureEngine::new(3, request.lags);

    Json(LagFeaturesResponse {
        lag_features: engine.create_lag_features(&request.values),
    })
}

async fn compute_growth_rate(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<GrowthRateRequest>,
) -> Json<GrowthRateResponse> {
    let engine = FeatureEngine::default();

    Json(GrowthRateResponse {
        growth_rates: engine.calculate_growth_rate(&request.values, request.periods),
    })
}

async fn ensemble_predict(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<EnsemblePredictRequest>,
) -> Json<EnsemblePredictResponse> {
    let weights = request.weights.unwrap_or_else(EnsembleWeights::uniform);
    let ensemble = StaticEnsemble::with_weights(weights.clone());
    let prediction = ensemble.predict(&request.predictions);

    Json(EnsemblePredictResponse {
        final_prediction: prediction,
        weights_used: weights,
    })
}

async fn optimize_ensemble(
    State(state): State<Arc<AppState>>,
    Json(request): Json<OptimizeEnsembleRequest>,
) -> Json<OptimizeEnsembleResponse> {
    let method = request.method.as_deref().unwrap_or("grid");

    let optimized_weights = match method {
        "gradient" => state
            .ensemble_optimiser
            .optimise_gradient_descent(&request.predictions, &request.actual),
        _ => state
            .ensemble_optimiser
            .optimise_grid_search(&request.predictions, &request.actual),
    };

    let metrics = state
        .ensemble_optimiser
        .evaluate(&request.predictions, &request.actual, &optimized_weights);

    Json(OptimizeEnsembleResponse {
        optimized_weights,
        metrics,
    })
}

async fn hash_data(Json(request): Json<HashDataRequest>) -> Json<HashDataResponse> {
    Json(HashDataResponse {
        hash: DataHasher::hash_string(&request.data),
    })
}

async fn create_commitment(
    Json(request): Json<CreateCommitmentRequest>,
) -> Json<CreateCommitmentResponse> {
    let commitment = CommitmentScheme::commit(
        &request.prediction_id,
        request.prediction,
        &request.model_hash,
        &request.input_hash,
        request.timestamp,
    );

    let salt = commitment.salt().to_string();
    Json(CreateCommitmentResponse {
        commitment_hash: commitment.commitment_hash,
        salt,
    })
}

async fn verify_commitment(
    Json(request): Json<VerifyCommitmentRequest>,
) -> Json<VerifyCommitmentResponse> {
    let valid = CommitmentScheme::verify(
        &request.commitment_hash,
        request.prediction,
        &request.salt,
        &request.model_hash,
        &request.input_hash,
    );

    Json(VerifyCommitmentResponse { valid })
}

async fn evaluate_predictions(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<EvaluateRequest>,
) -> Json<EvaluationMetrics> {
    let metrics = EvaluationMetrics::compute(&request.predictions, &request.actual);
    Json(metrics)
}

// --- Ingestion endpoint (ingestion feature) ---

#[cfg(feature = "ingestion")]
#[derive(Debug, Deserialize)]
struct IngestPricesRequest {
    coin_id: String,
    vs_currency: Option<String>,
    days: Option<u32>,
}

#[cfg(feature = "ingestion")]
async fn ingest_prices(
    State(state): State<Arc<AppState>>,
    Json(request): Json<IngestPricesRequest>,
) -> Json<Vec<crate::core::types::FeatureVector>> {
    use crate::ingestion::offchain_source::{OffChainConfig, OffChainSource};
    use crate::core::types::AssetClass;

    let source = OffChainSource::new(OffChainConfig::default());
    let vs = request.vs_currency.as_deref().unwrap_or("usd");
    let days = request.days.unwrap_or(7);

    let data = source
        .fetch_price_history(&request.coin_id, vs, days)
        .await
        .unwrap_or_default();

    if data.is_empty() {
        return Json(vec![]);
    }

    let timestamps: Vec<u64> = data.iter().map(|d| d.timestamp).collect();
    let prices: Vec<f64> = data.iter().map(|d| d.price).collect();
    let volumes: Vec<f64> = data.iter().map(|d| d.volume).collect();
    let liquidities: Vec<f64> = data.iter().map(|d| d.liquidity).collect();
    let tvls: Vec<Option<f64>> = data.iter().map(|d| d.tvl).collect();
    let borrow_rates: Vec<Option<f64>> = data.iter().map(|d| d.borrow_rate).collect();
    let utilisations: Vec<Option<f64>> = data.iter().map(|d| d.utilisation).collect();

    let features = state.feature_engine.generate_features(
        &data[0].asset_id,
        &timestamps,
        &prices,
        &volumes,
        &liquidities,
        &tvls,
        &borrow_rates,
        &utilisations,
        AssetClass::DeFiToken,
    );

    Json(features)
}

// --- Chain endpoints (substrate feature) ---

#[cfg(feature = "substrate")]
mod chain {
    use super::*;
    use axum::extract::Path;
    use axum::http::StatusCode;
    use crate::substrate_client::client::{format_h256, parse_h256};

    #[derive(Debug, Serialize)]
    pub struct TxResponse {
        pub tx_hash: String,
    }

    #[derive(Debug, Serialize)]
    pub struct ErrorResponse {
        pub error: String,
    }

    type ChainResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

    fn chain_err(e: impl std::fmt::Display) -> (StatusCode, Json<ErrorResponse>) {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() }))
    }

    fn no_client() -> (StatusCode, Json<ErrorResponse>) {
        (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse {
            error: "Substrate node not connected".to_string(),
        }))
    }

    fn get_client(state: &AppState) -> Result<&crate::substrate_client::client::SubstrateClient, (StatusCode, Json<ErrorResponse>)> {
        state.chain_client.as_ref().ok_or_else(no_client)
    }

    #[derive(Debug, Deserialize)]
    pub struct RegisterModelRequest {
        pub model_id: String,
        pub model_hash: String,
    }

    pub async fn register_model(
        State(state): State<Arc<AppState>>,
        Json(req): Json<RegisterModelRequest>,
    ) -> ChainResult<TxResponse> {
        let client = get_client(&state)?;
        let model_id = parse_h256(&req.model_id).map_err(chain_err)?;
        let model_hash = parse_h256(&req.model_hash).map_err(chain_err)?;
        let hash = client.register_model(model_id, model_hash).await.map_err(chain_err)?;
        Ok(Json(TxResponse { tx_hash: format_h256(&hash) }))
    }

    #[derive(Debug, Deserialize)]
    pub struct CreateMarketRequest {
        pub market_id: String,
        pub prediction_id: String,
    }

    pub async fn create_market(
        State(state): State<Arc<AppState>>,
        Json(req): Json<CreateMarketRequest>,
    ) -> ChainResult<TxResponse> {
        let client = get_client(&state)?;
        let market_id = parse_h256(&req.market_id).map_err(chain_err)?;
        let prediction_id = parse_h256(&req.prediction_id).map_err(chain_err)?;
        let hash = client.create_market(market_id, prediction_id).await.map_err(chain_err)?;
        Ok(Json(TxResponse { tx_hash: format_h256(&hash) }))
    }

    #[derive(Debug, Deserialize)]
    pub struct SubmitCommitmentRequest {
        pub prediction_id: String,
        pub commitment_hash: String,
    }

    pub async fn submit_commitment(
        State(state): State<Arc<AppState>>,
        Json(req): Json<SubmitCommitmentRequest>,
    ) -> ChainResult<TxResponse> {
        let client = get_client(&state)?;
        let prediction_id = parse_h256(&req.prediction_id).map_err(chain_err)?;
        let commitment_hash = parse_h256(&req.commitment_hash).map_err(chain_err)?;
        let hash = client.submit_commitment(prediction_id, commitment_hash).await.map_err(chain_err)?;
        Ok(Json(TxResponse { tx_hash: format_h256(&hash) }))
    }

    #[derive(Debug, Deserialize)]
    pub struct RevealRequest {
        pub prediction_id: String,
        pub prediction: i128,
        pub salt: String,
        pub model_hash: String,
        pub input_hash: String,
    }

    pub async fn reveal(
        State(state): State<Arc<AppState>>,
        Json(req): Json<RevealRequest>,
    ) -> ChainResult<TxResponse> {
        let client = get_client(&state)?;
        let prediction_id = parse_h256(&req.prediction_id).map_err(chain_err)?;
        let salt = parse_h256(&req.salt).map_err(chain_err)?;
        let model_hash = parse_h256(&req.model_hash).map_err(chain_err)?;
        let input_hash = parse_h256(&req.input_hash).map_err(chain_err)?;
        let hash = client
            .reveal_prediction(prediction_id, req.prediction, salt, model_hash, input_hash)
            .await
            .map_err(chain_err)?;
        Ok(Json(TxResponse { tx_hash: format_h256(&hash) }))
    }

    #[derive(Debug, Deserialize)]
    pub struct GroundTruthRequest {
        pub prediction_id: String,
        pub outcome: i128,
    }

    pub async fn ground_truth(
        State(state): State<Arc<AppState>>,
        Json(req): Json<GroundTruthRequest>,
    ) -> ChainResult<TxResponse> {
        let client = get_client(&state)?;
        let prediction_id = parse_h256(&req.prediction_id).map_err(chain_err)?;
        let hash = client.submit_ground_truth(prediction_id, req.outcome).await.map_err(chain_err)?;
        Ok(Json(TxResponse { tx_hash: format_h256(&hash) }))
    }

    #[derive(Debug, Deserialize)]
    pub struct StakeRequest {
        pub market_id: String,
        pub model_id: String,
        pub prediction_id: String,
        pub stake_amount: u128,
    }

    pub async fn stake(
        State(state): State<Arc<AppState>>,
        Json(req): Json<StakeRequest>,
    ) -> ChainResult<TxResponse> {
        let client = get_client(&state)?;
        let market_id = parse_h256(&req.market_id).map_err(chain_err)?;
        let model_id = parse_h256(&req.model_id).map_err(chain_err)?;
        let prediction_id = parse_h256(&req.prediction_id).map_err(chain_err)?;
        let hash = client
            .stake_prediction(market_id, model_id, prediction_id, req.stake_amount)
            .await
            .map_err(chain_err)?;
        Ok(Json(TxResponse { tx_hash: format_h256(&hash) }))
    }

    #[derive(Debug, Deserialize)]
    pub struct SettleRequest {
        pub market_id: String,
    }

    pub async fn settle(
        State(state): State<Arc<AppState>>,
        Json(req): Json<SettleRequest>,
    ) -> ChainResult<TxResponse> {
        let client = get_client(&state)?;
        let market_id = parse_h256(&req.market_id).map_err(chain_err)?;
        let hash = client.settle_market(market_id).await.map_err(chain_err)?;
        Ok(Json(TxResponse { tx_hash: format_h256(&hash) }))
    }

    #[derive(Debug, Serialize)]
    pub struct MarketResponse {
        pub status: String,
        pub total_stake: u128,
        pub participant_count: u32,
        pub created_block: u64,
        pub prediction_id: String,
    }

    pub async fn query_market(
        State(state): State<Arc<AppState>>,
        Path(id): Path<String>,
    ) -> ChainResult<Option<MarketResponse>> {
        let client = get_client(&state)?;
        let market_id = parse_h256(&id).map_err(chain_err)?;
        let market = client.query_market(market_id).await.map_err(chain_err)?;
        Ok(Json(market.map(|m| {
            let status = format!("{:?}", m.status);
            MarketResponse {
                status,
                total_stake: m.total_stake,
                participant_count: m.participant_count,
                created_block: m.created_block,
                prediction_id: format_h256(&m.prediction_id),
            }
        })))
    }
}

#[cfg(feature = "substrate")]
use chain::{
    register_model as chain_register_model,
    create_market as chain_create_market,
    submit_commitment as chain_submit_commitment,
    reveal as chain_reveal,
    ground_truth as chain_ground_truth,
    stake as chain_stake,
    settle as chain_settle,
    query_market as chain_query_market,
};
