"""
XGBoost model for time-series prediction.
Ported from football-rating-predictor — generalised for any tabular time-series domain.
"""

import numpy as np
import pandas as pd
from typing import Dict, List, Optional, Tuple, Any
from dataclasses import dataclass, field
import pickle
from pathlib import Path

import xgboost as xgb
from xgboost import XGBRegressor
from sklearn.model_selection import cross_val_score
from sklearn.metrics import mean_absolute_error, mean_squared_error, r2_score


@dataclass
class XGBoostConfig:
    n_estimators: int = 200
    max_depth: int = 6
    learning_rate: float = 0.1
    min_child_weight: int = 1
    subsample: float = 0.8
    colsample_bytree: float = 0.8
    gamma: float = 0
    reg_alpha: float = 0
    reg_lambda: float = 1
    early_stopping_rounds: int = 10
    random_state: int = 42

    use_lag_features: bool = True
    lag_periods: List[int] = field(default_factory=lambda: [1, 2, 3])
    use_rolling_features: bool = True
    rolling_window: int = 3
    cv_folds: int = 5


class XGBoostPredictor:
    """
    Domain-agnostic XGBoost predictor for tabular time-series.

    Expects a DataFrame with columns:
      - asset_id: identifier for the asset/entity
      - timestamp: ordering column (int or sortable)
      - target: the value to predict
      - feature columns: any numeric features
    """

    def __init__(self, config: Optional[XGBoostConfig] = None):
        self.config = config or XGBoostConfig()
        self.model: Optional[XGBRegressor] = None
        self.feature_columns: List[str] = []
        self.training_history: Dict[str, Any] = {}

    def _create_lag_features(
        self, df: pd.DataFrame, value_col: str = "target", group_col: str = "asset_id"
    ) -> pd.DataFrame:
        df = df.copy()
        df = df.sort_values([group_col, "timestamp"])
        for lag in self.config.lag_periods:
            df[f"{value_col}_lag_{lag}"] = df.groupby(group_col)[value_col].shift(lag)
        return df

    def _create_rolling_features(
        self, df: pd.DataFrame, value_col: str = "target", group_col: str = "asset_id"
    ) -> pd.DataFrame:
        df = df.copy()
        df = df.sort_values([group_col, "timestamp"])
        w = self.config.rolling_window

        df[f"{value_col}_rolling_mean_{w}"] = df.groupby(group_col)[
            value_col
        ].transform(lambda x: x.rolling(w, min_periods=1).mean().shift(1))

        df[f"{value_col}_rolling_std_{w}"] = df.groupby(group_col)[
            value_col
        ].transform(lambda x: x.rolling(w, min_periods=2).std().shift(1))

        df[f"{value_col}_rolling_min_{w}"] = df.groupby(group_col)[
            value_col
        ].transform(lambda x: x.rolling(w, min_periods=1).min().shift(1))

        df[f"{value_col}_rolling_max_{w}"] = df.groupby(group_col)[
            value_col
        ].transform(lambda x: x.rolling(w, min_periods=1).max().shift(1))

        return df

    def _create_growth_features(
        self, df: pd.DataFrame, value_col: str = "target", group_col: str = "asset_id"
    ) -> pd.DataFrame:
        df = df.copy()
        df = df.sort_values([group_col, "timestamp"])
        df[f"{value_col}_change_1"] = df.groupby(group_col)[value_col].diff(1)
        df[f"{value_col}_pct_change_1"] = (
            df.groupby(group_col)[value_col].pct_change(1) * 100
        )
        df[f"{value_col}_change_3"] = df.groupby(group_col)[value_col].diff(3)
        return df

    def prepare_features(
        self, df: pd.DataFrame, target_col: str = "target", fit: bool = True
    ) -> Tuple[pd.DataFrame, List[str]]:
        df = df.copy()

        if self.config.use_lag_features:
            df = self._create_lag_features(df, value_col=target_col)
        if self.config.use_rolling_features:
            df = self._create_rolling_features(df, value_col=target_col)
        df = self._create_growth_features(df, value_col=target_col)

        if fit:
            exclude = {"asset_id", "timestamp", target_col}
            self.feature_columns = [
                c for c in df.select_dtypes(include=[np.number]).columns if c not in exclude
            ]

        return df, self.feature_columns

    def fit(
        self,
        df: pd.DataFrame,
        target_col: str = "target",
        verbose: bool = True,
    ) -> "XGBoostPredictor":
        if verbose:
            print("=" * 60)
            print("FITTING XGBOOST MODEL")
            print("=" * 60)

        df_prepared, feature_cols = self.prepare_features(df, target_col=target_col, fit=True)
        df_prepared = df_prepared.dropna(subset=[target_col])

        X = df_prepared[feature_cols].fillna(0)
        y = df_prepared[target_col]

        if verbose:
            print(f"  Training samples: {len(X)}")
            print(f"  Features: {len(feature_cols)}")

        self.model = XGBRegressor(
            n_estimators=self.config.n_estimators,
            max_depth=self.config.max_depth,
            learning_rate=self.config.learning_rate,
            min_child_weight=self.config.min_child_weight,
            subsample=self.config.subsample,
            colsample_bytree=self.config.colsample_bytree,
            gamma=self.config.gamma,
            reg_alpha=self.config.reg_alpha,
            reg_lambda=self.config.reg_lambda,
            random_state=self.config.random_state,
            n_jobs=-1,
        )

        self.model.fit(X, y, eval_set=[(X, y)], verbose=False)

        if verbose:
            cv_scores = cross_val_score(
                self.model, X, y, cv=self.config.cv_folds, scoring="neg_mean_absolute_error"
            )
            print(f"  CV MAE: {-cv_scores.mean():.4f} (+/- {cv_scores.std():.4f})")
            print("  Model fitted.")

        self.training_history = {
            "n_samples": len(X),
            "n_features": len(feature_cols),
        }

        return self

    def predict(self, df: pd.DataFrame, target_col: str = "target") -> np.ndarray:
        if self.model is None:
            raise ValueError("Model not fitted.")

        df_prepared, _ = self.prepare_features(df, target_col=target_col, fit=False)
        X = df_prepared[self.feature_columns].fillna(0)
        return self.model.predict(X)

    def evaluate(self, df: pd.DataFrame, target_col: str = "target") -> Dict[str, float]:
        predictions = self.predict(df, target_col=target_col)
        actuals = df[target_col].values[: len(predictions)]

        mae = mean_absolute_error(actuals, predictions)
        rmse = np.sqrt(mean_squared_error(actuals, predictions))
        r2 = r2_score(actuals, predictions)

        return {"mae": mae, "rmse": rmse, "r2": r2, "n_samples": len(predictions)}

    def get_feature_importance(self, top_n: int = 15) -> pd.DataFrame:
        if self.model is None:
            raise ValueError("Model not fitted.")
        importance = self.model.feature_importances_
        return (
            pd.DataFrame({"feature": self.feature_columns, "importance": importance})
            .sort_values("importance", ascending=False)
            .head(top_n)
        )

    def save(self, filepath: str) -> None:
        Path(filepath).parent.mkdir(parents=True, exist_ok=True)
        with open(filepath, "wb") as f:
            pickle.dump(
                {
                    "model": self.model,
                    "config": self.config,
                    "feature_columns": self.feature_columns,
                },
                f,
            )

    @classmethod
    def load(cls, filepath: str) -> "XGBoostPredictor":
        with open(filepath, "rb") as f:
            data = pickle.load(f)
        predictor = cls(config=data["config"])
        predictor.model = data["model"]
        predictor.feature_columns = data["feature_columns"]
        return predictor
