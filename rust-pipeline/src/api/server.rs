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
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            feature_engine: FeatureEngine::default(),
            ensemble_optimiser: EnsembleOptimiser::default(),
        }
    }
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
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
        .route("/api/evaluate", post(evaluate_predictions))
        .layer(cors)
        .with_state(state)
}

pub async fn start_server(host: &str, port: u16) -> anyhow::Result<()> {
    let state = Arc::new(AppState::default());
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
