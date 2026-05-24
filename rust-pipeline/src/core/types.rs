use serde::{Deserialize, Serialize};

/// Market regime classification for context-aware prediction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass)]
pub enum MarketRegime {
    Trending,
    MeanReverting,
    Volatile,
    LowVolatility,
}

impl MarketRegime {
    pub fn from_volatility_and_trend(volatility: f64, trend_strength: f64) -> Self {
        if volatility > 0.5 {
            MarketRegime::Volatile
        } else if volatility < 0.1 {
            MarketRegime::LowVolatility
        } else if trend_strength.abs() > 0.3 {
            MarketRegime::Trending
        } else {
            MarketRegime::MeanReverting
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            MarketRegime::Trending => "Trending",
            MarketRegime::MeanReverting => "MeanReverting",
            MarketRegime::Volatile => "Volatile",
            MarketRegime::LowVolatility => "LowVolatility",
        }
    }
}

/// Asset class for context-aware weighting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass)]
pub enum AssetClass {
    DeFiToken,
    StablePair,
    LPToken,
    Derivative,
}

impl AssetClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssetClass::DeFiToken => "DeFiToken",
            AssetClass::StablePair => "StablePair",
            AssetClass::LPToken => "LPToken",
            AssetClass::Derivative => "Derivative",
        }
    }
}

/// Volatility level indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass)]
pub enum VolatilityLevel {
    Low,
    Medium,
    High,
    Extreme,
}

impl VolatilityLevel {
    pub fn from_value(realised_vol: f64) -> Self {
        match realised_vol {
            v if v < 0.1 => VolatilityLevel::Low,
            v if v < 0.3 => VolatilityLevel::Medium,
            v if v < 0.6 => VolatilityLevel::High,
            _ => VolatilityLevel::Extreme,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            VolatilityLevel::Low => "Low",
            VolatilityLevel::Medium => "Medium",
            VolatilityLevel::High => "High",
            VolatilityLevel::Extreme => "Extreme",
        }
    }
}

/// Prediction context for dynamic ensemble weighting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all, set_all))]
pub struct PredictionContext {
    pub asset_id: String,
    pub asset_class: AssetClass,
    pub market_regime: MarketRegime,
    pub volatility_level: VolatilityLevel,
    pub liquidity_depth: f64,
    pub observation_window: u64,
}

impl PredictionContext {
    pub fn new(
        asset_id: String,
        asset_class: AssetClass,
        volatility: f64,
        trend_strength: f64,
        liquidity_depth: f64,
        observation_window: u64,
    ) -> Self {
        PredictionContext {
            asset_id,
            asset_class,
            market_regime: MarketRegime::from_volatility_and_trend(volatility, trend_strength),
            volatility_level: VolatilityLevel::from_value(volatility),
            liquidity_depth,
            observation_window,
        }
    }
}

/// Raw time-series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    pub timestamp: u64,
    pub asset_id: String,
    pub price: f64,
    pub volume: f64,
    pub liquidity: f64,
    pub tvl: Option<f64>,
    pub borrow_rate: Option<f64>,
    pub utilisation: Option<f64>,
}

/// Generated feature vector for a single observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub asset_id: String,
    pub timestamp: u64,
    pub target: Option<f64>,

    // Price features
    pub price: f64,
    pub price_lag_1: Option<f64>,
    pub price_lag_2: Option<f64>,
    pub price_lag_3: Option<f64>,
    pub price_rolling_mean: Option<f64>,
    pub price_rolling_std: Option<f64>,

    // Momentum features
    pub momentum_1: Option<f64>,
    pub momentum_3: Option<f64>,
    pub rate_of_change_1: Option<f64>,
    pub rate_of_change_3: Option<f64>,

    // Volatility features
    pub realised_volatility: Option<f64>,
    pub volatility_ratio: Option<f64>,

    // Volume / liquidity features
    pub volume: f64,
    pub volume_rolling_mean: Option<f64>,
    pub liquidity: f64,
    pub liquidity_change: Option<f64>,

    // DeFi-specific
    pub tvl: Option<f64>,
    pub borrow_rate: Option<f64>,
    pub utilisation: Option<f64>,

    // Context
    pub market_regime: MarketRegime,
    pub asset_class: AssetClass,
}

/// Model predictions container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPredictions {
    pub asset_id: String,
    pub timestamp: u64,
    pub xgboost_prediction: f64,
    pub lstm_prediction: f64,
}

/// Ensemble weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleWeights {
    pub xgboost_weight: f64,
    pub lstm_weight: f64,
}

impl EnsembleWeights {
    pub fn uniform() -> Self {
        EnsembleWeights {
            xgboost_weight: 0.5,
            lstm_weight: 0.5,
        }
    }

    pub fn normalize(&mut self) {
        let sum = self.xgboost_weight + self.lstm_weight;
        if sum > 0.0 {
            self.xgboost_weight /= sum;
            self.lstm_weight /= sum;
        }
    }

    pub fn as_vec(&self) -> Vec<f64> {
        vec![self.xgboost_weight, self.lstm_weight]
    }

    pub fn from_vec(weights: Vec<f64>) -> Self {
        assert_eq!(weights.len(), 2);
        EnsembleWeights {
            xgboost_weight: weights[0],
            lstm_weight: weights[1],
        }
    }
}

/// Final prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub asset_id: String,
    pub timestamp: u64,
    pub predicted_value: f64,
    pub confidence: f64,
    pub model_predictions: ModelPredictions,
    pub weights_used: EnsembleWeights,
    pub context: PredictionContext,
    pub input_hash: String,
    pub commitment_hash: Option<String>,
}

/// Evaluation metrics (ported from old repo — domain-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationMetrics {
    pub mae: f64,
    pub rmse: f64,
    pub r_squared: f64,
    pub mape: f64,
    pub directional_accuracy: f64,
    pub sample_count: usize,
}

impl EvaluationMetrics {
    pub fn compute(predictions: &[f64], actual: &[f64]) -> Self {
        assert_eq!(predictions.len(), actual.len());
        let n = predictions.len() as f64;

        let mae = predictions
            .iter()
            .zip(actual.iter())
            .map(|(p, a)| (p - a).abs())
            .sum::<f64>()
            / n;

        let mse = predictions
            .iter()
            .zip(actual.iter())
            .map(|(p, a)| (p - a).powi(2))
            .sum::<f64>()
            / n;
        let rmse = mse.sqrt();

        let mean_actual = actual.iter().sum::<f64>() / n;
        let ss_tot = actual.iter().map(|a| (a - mean_actual).powi(2)).sum::<f64>();
        let ss_res = predictions
            .iter()
            .zip(actual.iter())
            .map(|(p, a)| (a - p).powi(2))
            .sum::<f64>();
        let r_squared = if ss_tot > 0.0 {
            1.0 - (ss_res / ss_tot)
        } else {
            0.0
        };

        let mape = predictions
            .iter()
            .zip(actual.iter())
            .filter(|(_, a)| **a != 0.0)
            .map(|(p, a)| ((p - a) / a).abs())
            .sum::<f64>()
            / n
            * 100.0;

        let directional_accuracy = if predictions.len() >= 2 {
            let correct = predictions
                .windows(2)
                .zip(actual.windows(2))
                .filter(|(p, a)| (p[1] - p[0]).signum() == (a[1] - a[0]).signum())
                .count();
            correct as f64 / (predictions.len() - 1) as f64 * 100.0
        } else {
            0.0
        };

        EvaluationMetrics {
            mae,
            rmse,
            r_squared,
            mape,
            directional_accuracy,
            sample_count: predictions.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_regime() {
        assert_eq!(
            MarketRegime::from_volatility_and_trend(0.7, 0.1),
            MarketRegime::Volatile
        );
        assert_eq!(
            MarketRegime::from_volatility_and_trend(0.05, 0.1),
            MarketRegime::LowVolatility
        );
        assert_eq!(
            MarketRegime::from_volatility_and_trend(0.3, 0.5),
            MarketRegime::Trending
        );
        assert_eq!(
            MarketRegime::from_volatility_and_trend(0.3, 0.1),
            MarketRegime::MeanReverting
        );
    }

    #[test]
    fn test_ensemble_weights() {
        let w = EnsembleWeights::uniform();
        assert!((w.xgboost_weight - 0.5).abs() < 0.01);
        assert!((w.lstm_weight - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_evaluation_metrics() {
        let predictions = vec![80.0, 85.0, 90.0];
        let actual = vec![82.0, 84.0, 88.0];

        let metrics = EvaluationMetrics::compute(&predictions, &actual);

        assert!(metrics.mae > 0.0);
        assert!(metrics.rmse > 0.0);
        assert!(metrics.r_squared >= 0.0 && metrics.r_squared <= 1.0);
    }
}
