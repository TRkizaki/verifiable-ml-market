//! # Prediction Market Pallet
//!
//! Tokenised prediction market where ML models compete.
//!
//! ## Flow
//! 1. Model operator registers a model via `register_model`
//! 2. Anyone creates a market round via `create_market`
//! 3. For each prediction round, operators stake tokens via `stake_prediction`
//! 4. After ground truth and reveals, anyone calls `settle_market`
//! 5. Accurate models earn rewards proportional to accuracy; inaccurate models lose stake

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{Currency, ReservableCurrency};
    use frame_system::pallet_prelude::*;
    use sp_core::H256;
    use alloc::vec::Vec;
    use sp_runtime::traits::SaturatedConversion;

    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    const MAX_PARTICIPANTS: u32 = 10;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_verification::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct ModelInfo<T: Config> {
        pub owner: T::AccountId,
        pub model_hash: H256,
        pub total_predictions: u64,
        pub total_correct: u64,
        pub reputation_score: u64,
        pub registered_block: BlockNumberFor<T>,
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct StakedPrediction<T: Config> {
        pub model_id: H256,
        pub owner: T::AccountId,
        pub prediction_id: H256,
        pub stake_amount: u128,
        pub settled: bool,
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq)]
    pub enum MarketStatus {
        Open,
        AwaitingReveal,
        AwaitingTruth,
        Settled,
    }

    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    pub struct MarketRound {
        pub status: MarketStatus,
        pub total_stake: u128,
        pub participant_count: u32,
        pub created_block: u64,
        pub prediction_id: H256,
    }

    #[pallet::storage]
    pub type Models<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, ModelInfo<T>>;

    #[pallet::storage]
    pub type Stakes<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, H256, Blake2_128Concat, H256, StakedPrediction<T>>;

    #[pallet::storage]
    pub type Markets<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, MarketRound>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ModelRegistered {
            model_id: H256,
            owner: T::AccountId,
        },
        MarketCreated {
            market_id: H256,
            creator: T::AccountId,
            prediction_id: H256,
        },
        PredictionStaked {
            market_id: H256,
            model_id: H256,
            stake_amount: u128,
        },
        MarketSettled {
            market_id: H256,
            total_distributed: u128,
        },
        RewardDistributed {
            market_id: H256,
            model_id: H256,
            owner: T::AccountId,
            amount: u128,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        ModelAlreadyRegistered,
        ModelNotFound,
        MarketNotFound,
        MarketNotOpen,
        MarketAlreadyExists,
        MarketAlreadySettled,
        InsufficientStake,
        AlreadyStaked,
        NotModelOwner,
        GroundTruthNotAvailable,
        TooManyParticipants,
        NoParticipants,
        RevealNotFound,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn register_model(
            origin: OriginFor<T>,
            model_id: H256,
            model_hash: H256,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                !Models::<T>::contains_key(model_id),
                Error::<T>::ModelAlreadyRegistered
            );

            let info = ModelInfo {
                owner: who.clone(),
                model_hash,
                total_predictions: 0,
                total_correct: 0,
                reputation_score: 5000,
                registered_block: <frame_system::Pallet<T>>::block_number(),
            };

            Models::<T>::insert(model_id, info);

            Self::deposit_event(Event::ModelRegistered {
                model_id,
                owner: who,
            });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn create_market(
            origin: OriginFor<T>,
            market_id: H256,
            prediction_id: H256,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                !Markets::<T>::contains_key(market_id),
                Error::<T>::MarketAlreadyExists
            );

            let block: u64 = <frame_system::Pallet<T>>::block_number().saturated_into();

            let market = MarketRound {
                status: MarketStatus::Open,
                total_stake: 0,
                participant_count: 0,
                created_block: block,
                prediction_id,
            };

            Markets::<T>::insert(market_id, market);

            Self::deposit_event(Event::MarketCreated {
                market_id,
                creator: who,
                prediction_id,
            });

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(15_000)]
        pub fn stake_prediction(
            origin: OriginFor<T>,
            market_id: H256,
            model_id: H256,
            prediction_id: H256,
            stake_amount: u128,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let model = Models::<T>::get(model_id)
                .ok_or(Error::<T>::ModelNotFound)?;
            ensure!(model.owner == who, Error::<T>::NotModelOwner);
            ensure!(stake_amount > 0, Error::<T>::InsufficientStake);

            let market = Markets::<T>::get(market_id)
                .ok_or(Error::<T>::MarketNotFound)?;
            ensure!(market.status == MarketStatus::Open, Error::<T>::MarketNotOpen);
            ensure!(market.participant_count < MAX_PARTICIPANTS, Error::<T>::TooManyParticipants);

            ensure!(
                !Stakes::<T>::contains_key(market_id, model_id),
                Error::<T>::AlreadyStaked
            );

            let balance: BalanceOf<T> = stake_amount.saturated_into();
            T::Currency::reserve(&who, balance)?;

            let staked = StakedPrediction {
                model_id,
                owner: who,
                prediction_id,
                stake_amount,
                settled: false,
            };

            Stakes::<T>::insert(market_id, model_id, staked);

            Markets::<T>::mutate(market_id, |maybe_market| {
                if let Some(market) = maybe_market {
                    market.total_stake += stake_amount;
                    market.participant_count += 1;
                }
            });

            Self::deposit_event(Event::PredictionStaked {
                market_id,
                model_id,
                stake_amount,
            });

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(50_000)]
        pub fn settle_market(
            origin: OriginFor<T>,
            market_id: H256,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let mut market = Markets::<T>::get(market_id)
                .ok_or(Error::<T>::MarketNotFound)?;

            ensure!(
                market.status != MarketStatus::Settled,
                Error::<T>::MarketAlreadySettled
            );
            ensure!(market.participant_count > 0, Error::<T>::NoParticipants);

            let ground_truth = pallet_verification::GroundTruths::<T>::get(market.prediction_id)
                .ok_or(Error::<T>::GroundTruthNotAvailable)?;

            let outcome = ground_truth.outcome;
            let total_pool = market.total_stake;

            // Collect all stakes and compute Brier scores
            // Brier score = (prediction - outcome)^2 / SCALE^2
            // Lower score = more accurate = higher reward
            let mut scores: Vec<(H256, T::AccountId, u128, u128)> = Vec::new();
            let mut total_inverse_score: u128 = 0;

            for (model_id, stake) in Stakes::<T>::iter_prefix(market_id) {
                let prediction_value = if let Some(reveal) =
                    pallet_verification::Reveals::<T>::get(stake.prediction_id)
                {
                    reveal.prediction
                } else {
                    // Model didn't reveal — treat as worst possible score
                    scores.push((model_id, stake.owner.clone(), stake.stake_amount, 0));
                    continue;
                };

                // Brier score: (prediction - outcome)^2 / 10^12
                // Both values are fixed-point * 10^6, so difference^2 is * 10^12
                let diff = prediction_value.saturating_sub(outcome);
                let brier_raw = (diff as i128).saturating_mul(diff as i128);
                let brier: u128 = (brier_raw.unsigned_abs()).saturating_div(1_000_000);

                // Inverse score for reward weighting (capped to avoid division by zero)
                // Higher inverse = better prediction
                let max_brier: u128 = 1_000_000_000_000;
                let inverse = max_brier.saturating_sub(brier.min(max_brier));

                total_inverse_score = total_inverse_score.saturating_add(inverse);
                scores.push((model_id, stake.owner.clone(), stake.stake_amount, inverse));
            }

            // Distribute rewards proportional to inverse Brier score
            let mut total_distributed: u128 = 0;

            for (model_id, owner, stake_amount, inverse_score) in scores.iter() {
                let balance: BalanceOf<T> = (*stake_amount).saturated_into();
                T::Currency::unreserve(&owner, balance);

                let reward = if total_inverse_score > 0 && *inverse_score > 0 {
                    total_pool
                        .saturating_mul(*inverse_score)
                        .saturating_div(total_inverse_score)
                } else {
                    0u128
                };

                if reward > 0 {
                    // Transfer reward from "pool" (mint for simplicity in demo)
                    // In production, this would come from the actual staked pool
                    let reward_balance: BalanceOf<T> = reward.saturated_into();
                    let _ = T::Currency::deposit_into_existing(&owner, reward_balance);
                    total_distributed = total_distributed.saturating_add(reward);

                    Self::deposit_event(Event::RewardDistributed {
                        market_id,
                        model_id: *model_id,
                        owner: owner.clone(),
                        amount: reward,
                    });
                }

                // Update model stats
                Models::<T>::mutate(model_id, |maybe_model| {
                    if let Some(model) = maybe_model {
                        model.total_predictions += 1;
                        if *inverse_score > 500_000_000_000 {
                            model.total_correct += 1;
                            model.reputation_score = model.reputation_score.saturating_add(100).min(10000);
                        } else {
                            model.reputation_score = model.reputation_score.saturating_sub(200);
                        }
                    }
                });

                // Mark stake as settled
                Stakes::<T>::mutate(market_id, model_id, |maybe_stake| {
                    if let Some(stake) = maybe_stake {
                        stake.settled = true;
                    }
                });
            }

            market.status = MarketStatus::Settled;
            Markets::<T>::insert(market_id, market);

            Self::deposit_event(Event::MarketSettled {
                market_id,
                total_distributed,
            });

            Ok(())
        }
    }
}
