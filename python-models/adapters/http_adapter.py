"""
HTTP adapter for Rust pipeline communication.
Ported from football-rating-predictor — updated endpoints.
"""

from typing import List, Optional, Dict, Any
import httpx

from .base import (
    RustAdapter,
    ModelPredictions,
    EnsembleWeights,
    EvaluationMetrics,
)


class HttpAdapter(RustAdapter):
    def __init__(self, base_url: str = "http://localhost:3000", timeout: float = 30.0):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self.client = httpx.Client(base_url=self.base_url, timeout=self.timeout)

    def health_check(self) -> Dict[str, str]:
        response = self.client.get("/health")
        response.raise_for_status()
        return response.json()

    def compute_rolling_features(
        self, values: List[float], window_size: int = 3
    ) -> Dict[str, List[Optional[float]]]:
        response = self.client.post(
            "/api/features/rolling",
            json={"values": values, "window_size": window_size},
        )
        response.raise_for_status()
        return response.json()

    def compute_lag_features(
        self, values: List[float], lags: List[int]
    ) -> List[List[Optional[float]]]:
        response = self.client.post(
            "/api/features/lag", json={"values": values, "lags": lags}
        )
        response.raise_for_status()
        return response.json()["lag_features"]

    def compute_growth_rate(
        self, values: List[float], periods: int = 1
    ) -> List[Optional[float]]:
        response = self.client.post(
            "/api/features/growth", json={"values": values, "periods": periods}
        )
        response.raise_for_status()
        return response.json()["growth_rates"]

    def ensemble_predict(
        self,
        predictions: ModelPredictions,
        weights: Optional[EnsembleWeights] = None,
    ) -> tuple[float, EnsembleWeights]:
        payload: Dict[str, Any] = {
            "predictions": {
                "asset_id": predictions.asset_id,
                "timestamp": predictions.timestamp,
                "xgboost_prediction": predictions.xgboost_prediction,
                "lstm_prediction": predictions.lstm_prediction,
            }
        }
        if weights:
            payload["weights"] = {
                "xgboost_weight": weights.xgboost_weight,
                "lstm_weight": weights.lstm_weight,
            }

        response = self.client.post("/api/ensemble/predict", json=payload)
        response.raise_for_status()
        data = response.json()
        return data["final_prediction"], EnsembleWeights.from_dict(data["weights_used"])

    def optimize_ensemble(
        self,
        predictions: List[ModelPredictions],
        actual: List[float],
        method: str = "grid",
    ) -> tuple[EnsembleWeights, EvaluationMetrics]:
        payload = {
            "predictions": [
                {
                    "asset_id": p.asset_id,
                    "timestamp": p.timestamp,
                    "xgboost_prediction": p.xgboost_prediction,
                    "lstm_prediction": p.lstm_prediction,
                }
                for p in predictions
            ],
            "actual": actual,
            "method": method,
        }

        response = self.client.post("/api/ensemble/optimize", json=payload)
        response.raise_for_status()
        data = response.json()
        return (
            EnsembleWeights.from_dict(data["optimized_weights"]),
            EvaluationMetrics.from_dict(data["metrics"]),
        )

    def hash_data(self, data: str) -> str:
        response = self.client.post("/api/provenance/hash", json={"data": data})
        response.raise_for_status()
        return response.json()["hash"]

    def create_commitment(
        self,
        prediction_id: str,
        prediction: float,
        model_hash: str,
        input_hash: str,
        timestamp: int,
    ) -> Dict[str, str]:
        response = self.client.post(
            "/api/provenance/commit",
            json={
                "prediction_id": prediction_id,
                "prediction": prediction,
                "model_hash": model_hash,
                "input_hash": input_hash,
                "timestamp": timestamp,
            },
        )
        response.raise_for_status()
        return response.json()

    def verify_commitment(
        self,
        commitment_hash: str,
        prediction: float,
        salt: str,
        model_hash: str,
        input_hash: str,
    ) -> bool:
        response = self.client.post(
            "/api/provenance/verify",
            json={
                "commitment_hash": commitment_hash,
                "prediction": prediction,
                "salt": salt,
                "model_hash": model_hash,
                "input_hash": input_hash,
            },
        )
        response.raise_for_status()
        return response.json()["valid"]

    def evaluate_predictions(
        self, predictions: List[float], actual: List[float]
    ) -> EvaluationMetrics:
        response = self.client.post(
            "/api/evaluate", json={"predictions": predictions, "actual": actual}
        )
        response.raise_for_status()
        return EvaluationMetrics.from_dict(response.json())
