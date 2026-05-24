use rayon::prelude::*;
use crate::core::types::*;

/// Static ensemble predictor (baseline)
pub struct StaticEnsemble {
    weights: EnsembleWeights,
}

impl StaticEnsemble {
    pub fn uniform() -> Self {
        StaticEnsemble {
            weights: EnsembleWeights::uniform(),
        }
    }

    pub fn with_weights(weights: EnsembleWeights) -> Self {
        StaticEnsemble { weights }
    }

    pub fn predict(&self, predictions: &ModelPredictions) -> f64 {
        self.weights.xgboost_weight * predictions.xgboost_prediction
            + self.weights.lstm_weight * predictions.lstm_prediction
    }

    pub fn weights(&self) -> &EnsembleWeights {
        &self.weights
    }
}

/// Ensemble weight optimiser using grid search and gradient descent.
/// Ported from football-rating-predictor — optimisation logic is domain-agnostic.
pub struct EnsembleOptimiser {
    learning_rate: f64,
    max_iterations: usize,
    tolerance: f64,
    grid_resolution: usize,
}

impl Default for EnsembleOptimiser {
    fn default() -> Self {
        EnsembleOptimiser {
            learning_rate: 0.01,
            max_iterations: 1000,
            tolerance: 1e-6,
            grid_resolution: 50,
        }
    }
}

impl EnsembleOptimiser {
    pub fn new(learning_rate: f64, max_iterations: usize, tolerance: f64) -> Self {
        EnsembleOptimiser {
            learning_rate,
            max_iterations,
            tolerance,
            grid_resolution: 50,
        }
    }

    pub fn with_grid_resolution(mut self, resolution: usize) -> Self {
        self.grid_resolution = resolution;
        self
    }

    /// Parallel grid search over weight space (2 models → 1D search)
    pub fn optimise_grid_search(
        &self,
        predictions: &[ModelPredictions],
        actual: &[f64],
    ) -> EnsembleWeights {
        let step = 1.0 / self.grid_resolution as f64;

        let combinations: Vec<[f64; 2]> = (0..=self.grid_resolution)
            .map(|i| {
                let w1 = i as f64 * step;
                let w2 = 1.0 - w1;
                [w1, w2]
            })
            .collect();

        let best = combinations
            .par_iter()
            .map(|weights| {
                let loss = self.compute_mse(predictions, actual, weights);
                (weights, loss)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        EnsembleWeights {
            xgboost_weight: best.0[0],
            lstm_weight: best.0[1],
        }
    }

    /// Gradient descent optimisation
    pub fn optimise_gradient_descent(
        &self,
        predictions: &[ModelPredictions],
        actual: &[f64],
    ) -> EnsembleWeights {
        let mut weights = [0.5, 0.5];

        for iteration in 0..self.max_iterations {
            let mut gradients = [0.0, 0.0];
            let mut total_loss = 0.0;

            for (pred, act) in predictions.iter().zip(actual.iter()) {
                let ensemble_pred = weights[0] * pred.xgboost_prediction
                    + weights[1] * pred.lstm_prediction;

                let error = ensemble_pred - act;
                total_loss += error * error;

                gradients[0] += 2.0 * error * pred.xgboost_prediction;
                gradients[1] += 2.0 * error * pred.lstm_prediction;
            }

            let n = predictions.len() as f64;
            for g in &mut gradients {
                *g /= n;
            }

            for i in 0..2 {
                weights[i] -= self.learning_rate * gradients[i];
                weights[i] = weights[i].max(0.0);
            }

            let sum: f64 = weights.iter().sum();
            if sum > 0.0 {
                for w in &mut weights {
                    *w /= sum;
                }
            }

            let avg_loss = total_loss / n;
            if avg_loss < self.tolerance {
                break;
            }

            if iteration > 100 {
                let gradient_norm =
                    (gradients[0].powi(2) + gradients[1].powi(2)).sqrt();
                if gradient_norm < self.tolerance {
                    break;
                }
            }
        }

        EnsembleWeights {
            xgboost_weight: weights[0],
            lstm_weight: weights[1],
        }
    }

    fn compute_mse(
        &self,
        predictions: &[ModelPredictions],
        actual: &[f64],
        weights: &[f64; 2],
    ) -> f64 {
        let mut total_error = 0.0;

        for (pred, act) in predictions.iter().zip(actual.iter()) {
            let ensemble_pred = weights[0] * pred.xgboost_prediction
                + weights[1] * pred.lstm_prediction;
            total_error += (ensemble_pred - act).powi(2);
        }

        total_error / predictions.len() as f64
    }

    pub fn evaluate(
        &self,
        predictions: &[ModelPredictions],
        actual: &[f64],
        weights: &EnsembleWeights,
    ) -> EvaluationMetrics {
        let ensemble = StaticEnsemble::with_weights(weights.clone());
        let ensemble_predictions: Vec<f64> =
            predictions.iter().map(|pred| ensemble.predict(pred)).collect();
        EvaluationMetrics::compute(&ensemble_predictions, actual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_predictions(xgboost: f64, lstm: f64) -> ModelPredictions {
        ModelPredictions {
            asset_id: "test".to_string(),
            timestamp: 0,
            xgboost_prediction: xgboost,
            lstm_prediction: lstm,
        }
    }

    #[test]
    fn test_static_ensemble() {
        let ensemble = StaticEnsemble::uniform();
        let predictions = create_test_predictions(82.0, 84.0);

        let result = ensemble.predict(&predictions);
        assert!((result - 83.0).abs() < 0.01);
    }

    #[test]
    fn test_grid_search_optimisation() {
        let optimiser = EnsembleOptimiser::default().with_grid_resolution(20);

        let predictions = vec![
            create_test_predictions(85.0, 82.0),
            create_test_predictions(78.0, 77.0),
            create_test_predictions(88.0, 91.0),
        ];
        let actual = vec![83.0, 77.0, 89.0];

        let weights = optimiser.optimise_grid_search(&predictions, &actual);

        let sum = weights.xgboost_weight + weights.lstm_weight;
        assert!((sum - 1.0).abs() < 0.01);
        assert!(weights.xgboost_weight >= 0.0);
        assert!(weights.lstm_weight >= 0.0);
    }
}
