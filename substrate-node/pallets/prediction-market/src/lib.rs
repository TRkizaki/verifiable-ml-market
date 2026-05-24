//! # Prediction Market Pallet
//!
//! Tokenised prediction market where ML models compete.
//!
//! ## Flow
//! 1. Model operator registers a model via `register_model`
//! 2. For each prediction round, operators stake tokens via `stake_prediction`
//! 3. After ground truth and reveals, anyone calls `settle_market`
//! 4. Accurate models earn rewards proportional to accuracy; inaccurate models lose stake
//!
//! ## Scoring
//! Uses Brier score (quadratic): lower = more accurate.
//! Rewards are distributed inversely proportional to Brier score.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_core::H256;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_verification::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    /// Registered ML model metadata
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct ModelInfo<T: Config> {
        pub owner: T::AccountId,
        pub model_hash: H256,
        pub total_predictions: u64,
        pub total_correct: u64,
        pub reputation_score: u64,     // 0-10000 (basis points)
        pub registered_block: BlockNumberFor<T>,
    }

    /// A staked prediction in a market round
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct StakedPrediction<T: Config> {
        pub model_id: H256,
        pub owner: T::AccountId,
        pub prediction_id: H256,
        pub stake_amount: u128,
        pub settled: bool,
    }

    /// Market round state
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
        PredictionStaked {
            market_id: H256,
            model_id: H256,
            stake_amount: u128,
        },
        MarketSettled {
            market_id: H256,
            total_distributed: u128,
        },
        RewardClaimed {
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
        MarketAlreadySettled,
        InsufficientStake,
        AlreadyStaked,
        NotModelOwner,
        GroundTruthNotAvailable,
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
                reputation_score: 5000,  // start at 50%
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

            ensure!(
                !Stakes::<T>::contains_key(market_id, model_id),
                Error::<T>::AlreadyStaked
            );

            // TODO: Actually transfer/lock tokens from the account
            // T::Currency::reserve(&who, stake_amount)?;

            let staked = StakedPrediction {
                model_id,
                owner: who,
                prediction_id,
                stake_amount,
                settled: false,
            };

            Stakes::<T>::insert(market_id, model_id, staked);

            // Update market round
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

        #[pallet::call_index(2)]
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

            // TODO: Iterate over all stakes for this market
            // For each stake: look up the revealed prediction and ground truth
            // Compute Brier score, distribute rewards proportionally
            // Slash inaccurate models

            market.status = MarketStatus::Settled;
            Markets::<T>::insert(market_id, market.clone());

            Self::deposit_event(Event::MarketSettled {
                market_id,
                total_distributed: market.total_stake,
            });

            Ok(())
        }
    }
}
