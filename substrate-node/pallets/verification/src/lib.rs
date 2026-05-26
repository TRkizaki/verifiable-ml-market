//! # Verification Pallet
//!
//! Commit-reveal scheme for verifiable ML predictions on-chain.
//!
//! ## Flow
//! 1. Model operator calls `submit_commitment(prediction_id, commitment_hash)`
//! 2. After ground truth is known, operator calls `reveal_prediction(prediction_id, prediction, salt, model_hash, input_hash)`
//! 3. Pallet verifies: Blake2(prediction || salt || model_hash || input_hash) == stored commitment
//! 4. Anyone can call `submit_ground_truth(prediction_id, outcome)` (oracle role)

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_core::H256;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct Commitment<T: Config> {
        pub submitter: T::AccountId,
        pub commitment_hash: H256,
        pub block_number: BlockNumberFor<T>,
        pub revealed: bool,
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct RevealedPrediction<T: Config> {
        pub submitter: T::AccountId,
        pub prediction: i128,
        pub model_hash: H256,
        pub input_hash: H256,
        pub block_number: BlockNumberFor<T>,
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct GroundTruth<T: Config> {
        pub outcome: i128,
        pub submitter: T::AccountId,
        pub block_number: BlockNumberFor<T>,
    }

    #[pallet::storage]
    pub type Commitments<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, Commitment<T>>;

    #[pallet::storage]
    pub type Reveals<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, RevealedPrediction<T>>;

    #[pallet::storage]
    pub type GroundTruths<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, GroundTruth<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CommitmentSubmitted {
            prediction_id: H256,
            submitter: T::AccountId,
        },
        PredictionRevealed {
            prediction_id: H256,
            submitter: T::AccountId,
            prediction: i128,
        },
        GroundTruthSubmitted {
            prediction_id: H256,
            outcome: i128,
        },
        VerificationFailed {
            prediction_id: H256,
            submitter: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        CommitmentAlreadyExists,
        CommitmentNotFound,
        AlreadyRevealed,
        VerificationMismatch,
        GroundTruthAlreadySubmitted,
        NotCommitmentOwner,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn submit_commitment(
            origin: OriginFor<T>,
            prediction_id: H256,
            commitment_hash: H256,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                !Commitments::<T>::contains_key(prediction_id),
                Error::<T>::CommitmentAlreadyExists
            );

            let commitment = Commitment {
                submitter: who.clone(),
                commitment_hash,
                block_number: <frame_system::Pallet<T>>::block_number(),
                revealed: false,
            };

            Commitments::<T>::insert(prediction_id, commitment);

            Self::deposit_event(Event::CommitmentSubmitted {
                prediction_id,
                submitter: who,
            });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(15_000)]
        pub fn reveal_prediction(
            origin: OriginFor<T>,
            prediction_id: H256,
            prediction: i128,
            salt: H256,
            model_hash: H256,
            input_hash: H256,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut commitment = Commitments::<T>::get(prediction_id)
                .ok_or(Error::<T>::CommitmentNotFound)?;

            ensure!(commitment.submitter == who, Error::<T>::NotCommitmentOwner);
            ensure!(!commitment.revealed, Error::<T>::AlreadyRevealed);

            // Verify: Blake2_256(prediction || salt || model_hash || input_hash) == commitment_hash
            let mut preimage = Vec::new();
            preimage.extend_from_slice(&prediction.to_le_bytes());
            preimage.extend_from_slice(salt.as_bytes());
            preimage.extend_from_slice(model_hash.as_bytes());
            preimage.extend_from_slice(input_hash.as_bytes());
            let computed_hash = H256::from(sp_core::hashing::blake2_256(&preimage));

            if computed_hash != commitment.commitment_hash {
                Self::deposit_event(Event::VerificationFailed {
                    prediction_id,
                    submitter: who,
                });
                return Err(Error::<T>::VerificationMismatch.into());
            }

            commitment.revealed = true;
            Commitments::<T>::insert(prediction_id, commitment);

            let revealed = RevealedPrediction {
                submitter: who.clone(),
                prediction,
                model_hash,
                input_hash,
                block_number: <frame_system::Pallet<T>>::block_number(),
            };
            Reveals::<T>::insert(prediction_id, revealed);

            Self::deposit_event(Event::PredictionRevealed {
                prediction_id,
                submitter: who,
                prediction,
            });

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn submit_ground_truth(
            origin: OriginFor<T>,
            prediction_id: H256,
            outcome: i128,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                !GroundTruths::<T>::contains_key(prediction_id),
                Error::<T>::GroundTruthAlreadySubmitted
            );

            let truth = GroundTruth {
                outcome,
                submitter: who,
                block_number: <frame_system::Pallet<T>>::block_number(),
            };

            GroundTruths::<T>::insert(prediction_id, truth);

            Self::deposit_event(Event::GroundTruthSubmitted {
                prediction_id,
                outcome,
            });

            Ok(())
        }
    }
}
