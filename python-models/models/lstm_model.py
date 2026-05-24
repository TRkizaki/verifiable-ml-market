"""
LSTM model for time-series prediction.
Ported from football-rating-predictor — generalised for any sequential prediction task.
Uses PyTorch instead of TensorFlow.
"""

import numpy as np
import pandas as pd
from typing import Dict, List, Optional, Tuple, Any
from dataclasses import dataclass, field
import pickle
from pathlib import Path

import torch
import torch.nn as nn
from torch.utils.data import Dataset, DataLoader


@dataclass
class LSTMConfig:
    sequence_length: int = 10
    lstm_units: List[int] = field(default_factory=lambda: [64, 32])
    dense_units: List[int] = field(default_factory=lambda: [16])
    dropout_rate: float = 0.2
    epochs: int = 100
    batch_size: int = 32
    learning_rate: float = 0.001
    validation_split: float = 0.2
    patience: int = 15

    sequence_features: List[str] = field(
        default_factory=lambda: ["target", "volume", "liquidity"]
    )

    random_state: int = 42


class TimeSeriesDataset(Dataset):
    def __init__(self, X: np.ndarray, y: np.ndarray):
        self.X = torch.FloatTensor(X)
        self.y = torch.FloatTensor(y)

    def __len__(self):
        return len(self.X)

    def __getitem__(self, idx):
        return self.X[idx], self.y[idx]


class LSTMNetwork(nn.Module):
    def __init__(self, n_features: int, config: LSTMConfig):
        super().__init__()

        layers = []
        input_size = n_features

        for i, units in enumerate(config.lstm_units):
            layers.append(
                nn.LSTM(
                    input_size=input_size,
                    hidden_size=units,
                    batch_first=True,
                    dropout=config.dropout_rate if i < len(config.lstm_units) - 1 else 0,
                )
            )
            input_size = units

        self.lstm_layers = nn.ModuleList(layers)
        self.dropout = nn.Dropout(config.dropout_rate)

        dense_layers = []
        prev_size = config.lstm_units[-1]
        for units in config.dense_units:
            dense_layers.append(nn.Linear(prev_size, units))
            dense_layers.append(nn.ReLU())
            dense_layers.append(nn.Dropout(config.dropout_rate))
            prev_size = units
        dense_layers.append(nn.Linear(prev_size, 1))

        self.dense = nn.Sequential(*dense_layers)

    def forward(self, x):
        for lstm in self.lstm_layers:
            x, _ = lstm(x)
        x = x[:, -1, :]
        x = self.dropout(x)
        return self.dense(x).squeeze(-1)


class LSTMPredictor:
    """
    Domain-agnostic LSTM predictor for sequential time-series.

    Expects a DataFrame with columns:
      - asset_id: identifier
      - timestamp: ordering column
      - target: the value to predict
      - additional numeric feature columns
    """

    def __init__(self, config: Optional[LSTMConfig] = None):
        self.config = config or LSTMConfig()
        self.model: Optional[LSTMNetwork] = None
        self.feature_columns: List[str] = []
        self.scalers: Dict[str, Tuple[float, float]] = {}
        self.training_history: Dict[str, Any] = {}
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")

        torch.manual_seed(self.config.random_state)
        np.random.seed(self.config.random_state)

    def _normalise(self, values: np.ndarray, col: str, fit: bool = True) -> np.ndarray:
        if fit:
            vmin, vmax = float(values.min()), float(values.max())
            if vmax == vmin:
                vmax = vmin + 1.0
            self.scalers[col] = (vmin, vmax)

        vmin, vmax = self.scalers[col]
        return (values - vmin) / (vmax - vmin)

    def _denormalise(self, values: np.ndarray, col: str) -> np.ndarray:
        vmin, vmax = self.scalers[col]
        return values * (vmax - vmin) + vmin

    def _prepare_sequences(
        self,
        df: pd.DataFrame,
        target_col: str = "target",
        group_col: str = "asset_id",
        fit: bool = True,
    ) -> Tuple[np.ndarray, np.ndarray, pd.DataFrame]:
        df = df.copy().sort_values([group_col, "timestamp"])

        available = [c for c in self.config.sequence_features if c in df.columns]
        if fit:
            self.feature_columns = available

        for col in self.feature_columns:
            df[f"{col}_norm"] = self._normalise(df[col].values, col, fit=fit)

        seq_len = self.config.sequence_length
        norm_cols = [f"{c}_norm" for c in self.feature_columns]

        sequences, targets, metadata = [], [], []

        for asset_id, group in df.groupby(group_col):
            group = group.sort_values("timestamp")
            if len(group) <= seq_len:
                continue

            for i in range(len(group) - seq_len):
                seq = group[norm_cols].iloc[i : i + seq_len].values.astype(float)
                target = group[f"{target_col}_norm"].iloc[i + seq_len]

                sequences.append(seq)
                targets.append(target)

                target_row = group.iloc[i + seq_len]
                metadata.append(
                    {
                        "asset_id": asset_id,
                        "timestamp": target_row["timestamp"],
                        "actual": target_row[target_col],
                    }
                )

        return np.array(sequences), np.array(targets), pd.DataFrame(metadata)

    def fit(
        self,
        df: pd.DataFrame,
        target_col: str = "target",
        verbose: bool = True,
    ) -> "LSTMPredictor":
        if verbose:
            print("=" * 60)
            print("FITTING LSTM MODEL")
            print("=" * 60)

        X, y, meta = self._prepare_sequences(df, target_col=target_col, fit=True)

        if verbose:
            print(f"  Sequences: {len(X)}, shape: {X.shape}")
            print(f"  Features: {self.feature_columns}")
            print(f"  Device: {self.device}")

        n_val = int(len(X) * self.config.validation_split)
        X_train, X_val = X[:-n_val], X[-n_val:]
        y_train, y_val = y[:-n_val], y[-n_val:]

        train_ds = TimeSeriesDataset(X_train, y_train)
        val_ds = TimeSeriesDataset(X_val, y_val)
        train_loader = DataLoader(train_ds, batch_size=self.config.batch_size, shuffle=True)
        val_loader = DataLoader(val_ds, batch_size=self.config.batch_size)

        n_features = X.shape[2]
        self.model = LSTMNetwork(n_features, self.config).to(self.device)
        optimizer = torch.optim.Adam(self.model.parameters(), lr=self.config.learning_rate)
        criterion = nn.MSELoss()
        scheduler = torch.optim.lr_scheduler.ReduceLROnPlateau(
            optimizer, patience=5, factor=0.5
        )

        best_val_loss = float("inf")
        patience_counter = 0
        train_losses, val_losses = [], []

        for epoch in range(self.config.epochs):
            self.model.train()
            epoch_loss = 0
            for X_batch, y_batch in train_loader:
                X_batch, y_batch = X_batch.to(self.device), y_batch.to(self.device)
                optimizer.zero_grad()
                pred = self.model(X_batch)
                loss = criterion(pred, y_batch)
                loss.backward()
                optimizer.step()
                epoch_loss += loss.item()

            train_loss = epoch_loss / len(train_loader)
            train_losses.append(train_loss)

            self.model.eval()
            val_loss = 0
            with torch.no_grad():
                for X_batch, y_batch in val_loader:
                    X_batch, y_batch = X_batch.to(self.device), y_batch.to(self.device)
                    pred = self.model(X_batch)
                    val_loss += criterion(pred, y_batch).item()

            val_loss /= len(val_loader)
            val_losses.append(val_loss)
            scheduler.step(val_loss)

            if val_loss < best_val_loss:
                best_val_loss = val_loss
                patience_counter = 0
                best_state = {k: v.cpu().clone() for k, v in self.model.state_dict().items()}
            else:
                patience_counter += 1
                if patience_counter >= self.config.patience:
                    if verbose:
                        print(f"  Early stopping at epoch {epoch + 1}")
                    break

        self.model.load_state_dict(best_state)

        self.training_history = {
            "train_loss": train_losses,
            "val_loss": val_losses,
            "epochs_trained": len(train_losses),
            "best_val_loss": best_val_loss,
        }

        if verbose:
            print(f"  Epochs trained: {len(train_losses)}")
            print(f"  Best val loss: {best_val_loss:.6f}")
            print("  Model fitted.")

        return self

    def predict(self, df: pd.DataFrame, target_col: str = "target") -> np.ndarray:
        if self.model is None:
            raise ValueError("Model not fitted.")

        X, _, meta = self._prepare_sequences(df, target_col=target_col, fit=False)
        if len(X) == 0:
            return np.array([])

        self.model.eval()
        with torch.no_grad():
            X_tensor = torch.FloatTensor(X).to(self.device)
            predictions_norm = self.model(X_tensor).cpu().numpy()

        return self._denormalise(predictions_norm, target_col)

    def evaluate(self, df: pd.DataFrame, target_col: str = "target") -> Dict[str, float]:
        X, y, meta = self._prepare_sequences(df, target_col=target_col, fit=False)
        if len(X) == 0:
            return {"mae": float("inf"), "rmse": float("inf"), "r2": 0.0}

        self.model.eval()
        with torch.no_grad():
            X_tensor = torch.FloatTensor(X).to(self.device)
            pred_norm = self.model(X_tensor).cpu().numpy()

        predictions = self._denormalise(pred_norm, target_col)
        actuals = meta["actual"].values

        mae = np.mean(np.abs(predictions - actuals))
        rmse = np.sqrt(np.mean((predictions - actuals) ** 2))
        ss_res = np.sum((actuals - predictions) ** 2)
        ss_tot = np.sum((actuals - np.mean(actuals)) ** 2)
        r2 = 1 - (ss_res / ss_tot) if ss_tot > 0 else 0

        return {"mae": mae, "rmse": rmse, "r2": r2, "n_samples": len(predictions)}

    def save(self, filepath: str) -> None:
        Path(filepath).parent.mkdir(parents=True, exist_ok=True)
        torch.save(
            {
                "model_state": self.model.state_dict(),
                "config": self.config,
                "feature_columns": self.feature_columns,
                "scalers": self.scalers,
            },
            filepath,
        )

    @classmethod
    def load(cls, filepath: str) -> "LSTMPredictor":
        data = torch.load(filepath, weights_only=False)
        predictor = cls(config=data["config"])
        predictor.feature_columns = data["feature_columns"]
        predictor.scalers = data["scalers"]

        n_features = len(predictor.feature_columns)
        predictor.model = LSTMNetwork(n_features, predictor.config)
        predictor.model.load_state_dict(data["model_state"])
        return predictor
