use rayon::prelude::*;
use crate::core::types::*;

/// Domain-agnostic time-series feature engineering engine.
/// Ported from football-rating-predictor — all operations are generic.
pub struct FeatureEngine {
    window_size: usize,
    lag_periods: Vec<usize>,
}

impl Default for FeatureEngine {
    fn default() -> Self {
        FeatureEngine {
            window_size: 3,
            lag_periods: vec![1, 2, 3],
        }
    }
}

impl FeatureEngine {
    pub fn new(window_size: usize, lag_periods: Vec<usize>) -> Self {
        FeatureEngine {
            window_size,
            lag_periods,
        }
    }

    pub fn rolling_mean(&self, values: &[f64]) -> Vec<Option<f64>> {
        values
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i < self.window_size - 1 {
                    None
                } else {
                    let window = &values[i + 1 - self.window_size..=i];
                    Some(window.iter().sum::<f64>() / self.window_size as f64)
                }
            })
            .collect()
    }

    pub fn rolling_std(&self, values: &[f64]) -> Vec<Option<f64>> {
        let means = self.rolling_mean(values);

        values
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i < self.window_size - 1 {
                    None
                } else {
                    let mean = means[i]?;
                    let window = &values[i + 1 - self.window_size..=i];
                    let variance = window
                        .iter()
                        .map(|x| (x - mean).powi(2))
                        .sum::<f64>()
                        / self.window_size as f64;
                    Some(variance.sqrt())
                }
            })
            .collect()
    }

    pub fn rolling_min(&self, values: &[f64]) -> Vec<Option<f64>> {
        values
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i < self.window_size - 1 {
                    None
                } else {
                    let window = &values[i + 1 - self.window_size..=i];
                    window.iter().cloned().reduce(f64::min)
                }
            })
            .collect()
    }

    pub fn rolling_max(&self, values: &[f64]) -> Vec<Option<f64>> {
        values
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i < self.window_size - 1 {
                    None
                } else {
                    let window = &values[i + 1 - self.window_size..=i];
                    window.iter().cloned().reduce(f64::max)
                }
            })
            .collect()
    }

    pub fn create_lag_features(&self, values: &[f64]) -> Vec<Vec<Option<f64>>> {
        self.lag_periods
            .iter()
            .map(|&lag| {
                values
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        if i < lag {
                            None
                        } else {
                            Some(values[i - lag])
                        }
                    })
                    .collect()
            })
            .collect()
    }

    pub fn calculate_growth_rate(&self, values: &[f64], periods: usize) -> Vec<Option<f64>> {
        values
            .iter()
            .enumerate()
            .map(|(i, current)| {
                if i < periods {
                    None
                } else {
                    let previous = values[i - periods];
                    if previous != 0.0 {
                        Some((current - previous) / previous * 100.0)
                    } else {
                        None
                    }
                }
            })
            .collect()
    }

    pub fn calculate_momentum(&self, values: &[f64], periods: usize) -> Vec<Option<f64>> {
        values
            .iter()
            .enumerate()
            .map(|(i, current)| {
                if i < periods {
                    None
                } else {
                    Some(current - values[i - periods])
                }
            })
            .collect()
    }

    /// Realised volatility over a rolling window (annualised std of returns)
    pub fn realised_volatility(&self, prices: &[f64]) -> Vec<Option<f64>> {
        if prices.len() < 2 {
            return vec![None; prices.len()];
        }

        let returns: Vec<f64> = prices
            .windows(2)
            .map(|w| if w[0] != 0.0 { (w[1] / w[0]).ln() } else { 0.0 })
            .collect();

        let engine = FeatureEngine::new(self.window_size, vec![]);
        let stds = engine.rolling_std(&returns);

        let mut result = vec![None];
        for std in stds {
            result.push(std);
        }
        result
    }

    pub fn calculate_consistency(&self, values: &[f64]) -> Option<f64> {
        if values.len() < 2 {
            return None;
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        if mean == 0.0 {
            return None;
        }

        let variance = values
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;
        let std_dev = variance.sqrt();

        let cv = std_dev / mean;
        Some(1.0 / (1.0 + cv))
    }

    /// Exponential moving average
    pub fn ema(&self, values: &[f64], span: usize) -> Vec<Option<f64>> {
        if values.is_empty() {
            return vec![];
        }

        let alpha = 2.0 / (span as f64 + 1.0);
        let mut result = Vec::with_capacity(values.len());
        let mut ema_val = values[0];

        for (i, &val) in values.iter().enumerate() {
            if i == 0 {
                result.push(None);
            } else {
                ema_val = alpha * val + (1.0 - alpha) * ema_val;
                result.push(Some(ema_val));
            }
        }
        result
    }

    /// Generate full feature vector for a time series of an asset
    pub fn generate_features(
        &self,
        asset_id: &str,
        timestamps: &[u64],
        prices: &[f64],
        volumes: &[f64],
        liquidities: &[f64],
        tvls: &[Option<f64>],
        borrow_rates: &[Option<f64>],
        utilisations: &[Option<f64>],
        asset_class: AssetClass,
    ) -> Vec<FeatureVector> {
        let n = prices.len();

        let rolling_means = self.rolling_mean(prices);
        let rolling_stds = self.rolling_std(prices);
        let lag_features = self.create_lag_features(prices);
        let momentum_1 = self.calculate_momentum(prices, 1);
        let momentum_3 = self.calculate_momentum(prices, 3);
        let roc_1 = self.calculate_growth_rate(prices, 1);
        let roc_3 = self.calculate_growth_rate(prices, 3);
        let vol = self.realised_volatility(prices);
        let vol_rolling = self.rolling_mean(volumes);
        let liq_change = self.calculate_growth_rate(liquidities, 1);

        (0..n)
            .map(|i| {
                let realised_vol = vol[i].unwrap_or(0.3);
                let trend = momentum_3[i].unwrap_or(0.0)
                    / (rolling_stds[i].unwrap_or(1.0).max(0.001));
                let regime =
                    MarketRegime::from_volatility_and_trend(realised_vol, trend);

                FeatureVector {
                    asset_id: asset_id.to_string(),
                    timestamp: timestamps[i],
                    target: if i + 1 < n {
                        Some(prices[i + 1])
                    } else {
                        None
                    },
                    price: prices[i],
                    price_lag_1: lag_features.first().and_then(|v| v[i]),
                    price_lag_2: lag_features.get(1).and_then(|v| v[i]),
                    price_lag_3: lag_features.get(2).and_then(|v| v[i]),
                    price_rolling_mean: rolling_means[i],
                    price_rolling_std: rolling_stds[i],
                    momentum_1: momentum_1[i],
                    momentum_3: momentum_3[i],
                    rate_of_change_1: roc_1[i],
                    rate_of_change_3: roc_3[i],
                    realised_volatility: vol[i],
                    volatility_ratio: vol[i].and_then(|v| {
                        rolling_stds[i].map(|s| if s > 0.0 { v / s } else { 0.0 })
                    }),
                    volume: volumes[i],
                    volume_rolling_mean: vol_rolling[i],
                    liquidity: liquidities[i],
                    liquidity_change: liq_change[i],
                    tvl: tvls[i],
                    borrow_rate: borrow_rates[i],
                    utilisation: utilisations[i],
                    market_regime: regime,
                    asset_class,
                }
            })
            .collect()
    }

    pub fn process_assets_parallel<F, T>(&self, asset_ids: &[String], processor: F) -> Vec<T>
    where
        F: Fn(&str) -> T + Sync + Send,
        T: Send,
    {
        asset_ids.par_iter().map(|id| processor(id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_mean() {
        let engine = FeatureEngine::new(3, vec![1, 2]);
        let values = vec![80.0, 82.0, 84.0, 86.0, 88.0];

        let result = engine.rolling_mean(&values);

        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!((result[2].unwrap() - 82.0).abs() < 0.01);
        assert!((result[3].unwrap() - 84.0).abs() < 0.01);
        assert!((result[4].unwrap() - 86.0).abs() < 0.01);
    }

    #[test]
    fn test_lag_features() {
        let engine = FeatureEngine::new(3, vec![1, 2]);
        let values = vec![80.0, 82.0, 84.0, 86.0];

        let result = engine.create_lag_features(&values);

        assert!(result[0][0].is_none());
        assert_eq!(result[0][1], Some(80.0));
        assert_eq!(result[0][2], Some(82.0));

        assert!(result[1][0].is_none());
        assert!(result[1][1].is_none());
        assert_eq!(result[1][2], Some(80.0));
    }

    #[test]
    fn test_growth_rate() {
        let engine = FeatureEngine::default();
        let values = vec![80.0, 84.0, 88.0];

        let result = engine.calculate_growth_rate(&values, 1);

        assert!(result[0].is_none());
        assert!((result[1].unwrap() - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_momentum() {
        let engine = FeatureEngine::default();
        let values = vec![100.0, 102.0, 105.0, 103.0];

        let result = engine.calculate_momentum(&values, 1);

        assert!(result[0].is_none());
        assert!((result[1].unwrap() - 2.0).abs() < 0.01);
        assert!((result[2].unwrap() - 3.0).abs() < 0.01);
        assert!((result[3].unwrap() - (-2.0)).abs() < 0.01);
    }

    #[test]
    fn test_consistency() {
        let engine = FeatureEngine::default();

        let consistent = vec![80.0, 81.0, 80.0, 81.0];
        let inconsistent = vec![70.0, 90.0, 60.0, 100.0];

        let c1 = engine.calculate_consistency(&consistent).unwrap();
        let c2 = engine.calculate_consistency(&inconsistent).unwrap();

        assert!(c1 > c2);
    }
}
