pub mod core;
pub mod ingestion;
pub mod provenance;

#[cfg(feature = "http")]
pub mod api;

#[cfg(feature = "python")]
pub mod python;

#[cfg(feature = "substrate")]
pub mod substrate_client;

pub use core::{
    // Types
    MarketRegime, AssetClass, VolatilityLevel, PredictionContext,
    TimeSeriesData, FeatureVector, ModelPredictions, EnsembleWeights,
    PredictionResult, EvaluationMetrics,

    // Feature engineering
    FeatureEngine,

    // Ensemble
    StaticEnsemble, EnsembleOptimiser,
};

pub use provenance::{
    DataHasher, Commitment, CommitmentScheme, MerkleTree,
};
