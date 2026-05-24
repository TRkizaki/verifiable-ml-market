"""
Base adapter interface for Rust pipeline communication.
Ported from football-rating-predictor — generalised interface.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import List, Optional, Dict, Any
import numpy as np


@dataclass
class ModelPredictions:
    asset_id: str
    timestamp: int
    xgboost_prediction: float
    lstm_prediction: float


@dataclass
class EnsembleWeights:
    xgboost_weight: float
    lstm_weight: float

    def as_list(self) -> List[float]:
        return [self.xgboost_weight, self.lstm_weight]

    @classmethod
    def from_dict(cls, d: Dict[str, float]) -> "EnsembleWeights":
        return cls(
            xgboost_weight=d.get("xgboost_weight", 0.5),
            lstm_weight=d.get("lstm_weight", 0.5),
        )


@dataclass
class EvaluationMetrics:
    mae: float
    rmse: float
    r_squared: float
    mape: float
    directional_accuracy: float
    sample_count: int

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> "EvaluationMetrics":
        return cls(
            mae=d["mae"],
            rmse=d["rmse"],
            r_squared=d["r_squared"],
            mape=d["mape"],
            directional_accuracy=d.get("directional_accuracy", 0.0),
            sample_count=d["sample_count"],
        )


class RustAdapter(ABC):
    """
    Abstract base class for Rust pipeline communication.
    Implementations: HttpAdapter (HTTP API), PyO3Adapter (direct bindings).
    """

    @abstractmethod
    def health_check(self) -> Dict[str, str]:
        pass

    @abstractmethod
    def compute_rolling_features(
        self, values: List[float], window_size: int = 3
    ) -> Dict[str, List[Optional[float]]]:
        pass

    @abstractmethod
    def compute_lag_features(
        self, values: List[float], lags: List[int]
    ) -> List[List[Optional[float]]]:
        pass

    @abstractmethod
    def compute_growth_rate(
        self, values: List[float], periods: int = 1
    ) -> List[Optional[float]]:
        pass

    @abstractmethod
    def ensemble_predict(
        self,
        predictions: ModelPredictions,
        weights: Optional[EnsembleWeights] = None,
    ) -> tuple[float, EnsembleWeights]:
        pass

    @abstractmethod
    def optimize_ensemble(
        self,
        predictions: List[ModelPredictions],
        actual: List[float],
        method: str = "grid",
    ) -> tuple[EnsembleWeights, EvaluationMetrics]:
        pass

    @abstractmethod
    def hash_data(self, data: str) -> str:
        pass

    @abstractmethod
    def create_commitment(
        self,
        prediction_id: str,
        prediction: float,
        model_hash: str,
        input_hash: str,
        timestamp: int,
    ) -> Dict[str, str]:
        pass

    @abstractmethod
    def verify_commitment(
        self,
        commitment_hash: str,
        prediction: float,
        salt: str,
        model_hash: str,
        input_hash: str,
    ) -> bool:
        pass

    @abstractmethod
    def evaluate_predictions(
        self, predictions: List[float], actual: List[float]
    ) -> EvaluationMetrics:
        pass


def create_adapter(backend: str = "http", **kwargs) -> RustAdapter:
    if backend == "http":
        from .http_adapter import HttpAdapter
        return HttpAdapter(**kwargs)
    else:
        raise ValueError(f"Unknown backend: {backend}. Use 'http'.")
