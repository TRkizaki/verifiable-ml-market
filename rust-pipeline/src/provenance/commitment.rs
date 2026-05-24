use serde::{Deserialize, Serialize};
use crate::provenance::hasher::DataHasher;

/// A commitment: Hash(prediction || salt || model_hash || input_hash)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    pub commitment_hash: String,
    pub prediction_id: String,
    pub timestamp: u64,
    /// Kept secret until reveal phase
    salt: String,
    prediction: f64,
    model_hash: String,
    input_hash: String,
}

impl Commitment {
    pub fn prediction(&self) -> f64 {
        self.prediction
    }

    pub fn salt(&self) -> &str {
        &self.salt
    }

    pub fn model_hash(&self) -> &str {
        &self.model_hash
    }

    pub fn input_hash(&self) -> &str {
        &self.input_hash
    }
}

/// Commit-reveal scheme for verifiable predictions.
pub struct CommitmentScheme;

impl CommitmentScheme {
    /// Create a commitment for a prediction.
    /// Returns the commitment (hash stored on-chain) and the reveal data (kept secret).
    pub fn commit(
        prediction_id: &str,
        prediction: f64,
        model_hash: &str,
        input_hash: &str,
        timestamp: u64,
    ) -> Commitment {
        let salt = Self::generate_salt();
        let commitment_hash = Self::compute_hash(prediction, &salt, model_hash, input_hash);

        Commitment {
            commitment_hash,
            prediction_id: prediction_id.to_string(),
            timestamp,
            salt,
            prediction,
            model_hash: model_hash.to_string(),
            input_hash: input_hash.to_string(),
        }
    }

    /// Verify a revealed prediction against a commitment hash.
    pub fn verify(
        commitment_hash: &str,
        prediction: f64,
        salt: &str,
        model_hash: &str,
        input_hash: &str,
    ) -> bool {
        let computed = Self::compute_hash(prediction, salt, model_hash, input_hash);
        computed == commitment_hash
    }

    fn compute_hash(prediction: f64, salt: &str, model_hash: &str, input_hash: &str) -> String {
        let preimage = format!(
            "{}|{}|{}|{}",
            prediction, salt, model_hash, input_hash
        );
        DataHasher::hash_string(&preimage)
    }

    fn generate_salt() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let salt_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        hex::encode(salt_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_and_verify() {
        let commitment = CommitmentScheme::commit(
            "pred_001",
            42.5,
            "model_hash_abc",
            "input_hash_def",
            1700000000,
        );

        assert!(CommitmentScheme::verify(
            &commitment.commitment_hash,
            commitment.prediction(),
            commitment.salt(),
            commitment.model_hash(),
            commitment.input_hash(),
        ));
    }

    #[test]
    fn test_tampered_prediction_fails() {
        let commitment = CommitmentScheme::commit(
            "pred_002",
            42.5,
            "model_hash_abc",
            "input_hash_def",
            1700000000,
        );

        assert!(!CommitmentScheme::verify(
            &commitment.commitment_hash,
            99.9, // tampered
            commitment.salt(),
            commitment.model_hash(),
            commitment.input_hash(),
        ));
    }

    #[test]
    fn test_different_salts_different_hashes() {
        let c1 = CommitmentScheme::commit("p1", 42.5, "m", "i", 0);
        let c2 = CommitmentScheme::commit("p2", 42.5, "m", "i", 0);

        assert_ne!(c1.commitment_hash, c2.commitment_hash);
    }
}
