use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

/// Cryptographic hasher for data provenance.
/// Produces SHA-256 hashes of input data, model versions, and feature vectors.
pub struct DataHasher;

impl DataHasher {
    pub fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    pub fn hash_string(data: &str) -> String {
        Self::hash_bytes(data.as_bytes())
    }

    /// Hash a serialisable value (feature vector, prediction, model config)
    pub fn hash_value<T: Serialize>(value: &T) -> anyhow::Result<String> {
        let json = serde_json::to_string(value)?;
        Ok(Self::hash_string(&json))
    }

    /// Hash multiple values and combine into a single digest
    pub fn hash_combined(parts: &[&str]) -> String {
        let combined = parts.join("|");
        Self::hash_string(&combined)
    }
}

/// Merkle tree for batch provenance of multiple predictions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    pub root: String,
    pub leaves: Vec<String>,
    pub levels: Vec<Vec<String>>,
}

impl MerkleTree {
    pub fn from_data(items: &[String]) -> Self {
        let leaves: Vec<String> = items.iter().map(|s| DataHasher::hash_string(s)).collect();

        if leaves.is_empty() {
            return MerkleTree {
                root: DataHasher::hash_string("empty"),
                leaves: vec![],
                levels: vec![],
            };
        }

        let mut levels = vec![leaves.clone()];
        let mut current_level = leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let combined = if chunk.len() == 2 {
                    DataHasher::hash_combined(&[&chunk[0], &chunk[1]])
                } else {
                    DataHasher::hash_combined(&[&chunk[0], &chunk[0]])
                };
                next_level.push(combined);
            }

            levels.push(next_level.clone());
            current_level = next_level;
        }

        MerkleTree {
            root: current_level[0].clone(),
            leaves: items.iter().map(|s| DataHasher::hash_string(s)).collect(),
            levels,
        }
    }

    pub fn verify_leaf(&self, index: usize, data: &str) -> bool {
        if index >= self.leaves.len() {
            return false;
        }
        let hash = DataHasher::hash_string(data);
        hash == self.leaves[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_deterministic() {
        let h1 = DataHasher::hash_string("test data");
        let h2 = DataHasher::hash_string("test data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_different_inputs() {
        let h1 = DataHasher::hash_string("input A");
        let h2 = DataHasher::hash_string("input B");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_merkle_tree() {
        let items: Vec<String> = vec!["a", "b", "c", "d"]
            .into_iter()
            .map(String::from)
            .collect();
        let tree = MerkleTree::from_data(&items);

        assert!(!tree.root.is_empty());
        assert_eq!(tree.leaves.len(), 4);
        assert!(tree.verify_leaf(0, "a"));
        assert!(!tree.verify_leaf(0, "b"));
    }

    #[test]
    fn test_merkle_tree_odd_count() {
        let items: Vec<String> = vec!["a", "b", "c"]
            .into_iter()
            .map(String::from)
            .collect();
        let tree = MerkleTree::from_data(&items);

        assert!(!tree.root.is_empty());
        assert_eq!(tree.leaves.len(), 3);
    }
}
