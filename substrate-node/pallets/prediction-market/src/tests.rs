use crate::{mock::*, pallet::*, Error, Event};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

fn make_commitment_hash(prediction: i128, salt: H256, model_hash: H256, input_hash: H256) -> H256 {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(&prediction.to_le_bytes());
    preimage.extend_from_slice(salt.as_bytes());
    preimage.extend_from_slice(model_hash.as_bytes());
    preimage.extend_from_slice(input_hash.as_bytes());
    H256::from(sp_core::hashing::blake2_256(&preimage))
}

#[test]
fn register_model_stores_with_reputation_5000() {
    new_test_ext().execute_with(|| {
        let model_id = H256::from_low_u64_be(1);
        let model_hash = H256::from_low_u64_be(100);

        assert_ok!(PredictionMarket::register_model(
            RuntimeOrigin::signed(1),
            model_id,
            model_hash,
        ));

        let model = Models::<Test>::get(model_id).unwrap();
        assert_eq!(model.owner, 1);
        assert_eq!(model.model_hash, model_hash);
        assert_eq!(model.reputation_score, 5000);
        assert_eq!(model.total_predictions, 0);
        assert_eq!(model.total_correct, 0);

        System::assert_last_event(
            Event::<Test>::ModelRegistered {
                model_id,
                owner: 1,
            }
            .into(),
        );
    });
}

#[test]
fn duplicate_model_rejected() {
    new_test_ext().execute_with(|| {
        let model_id = H256::from_low_u64_be(1);
        let model_hash = H256::from_low_u64_be(100);

        assert_ok!(PredictionMarket::register_model(
            RuntimeOrigin::signed(1),
            model_id,
            model_hash,
        ));

        assert_noop!(
            PredictionMarket::register_model(
                RuntimeOrigin::signed(2),
                model_id,
                model_hash,
            ),
            Error::<Test>::ModelAlreadyRegistered
        );
    });
}

#[test]
fn create_market_emits_event() {
    new_test_ext().execute_with(|| {
        let market_id = H256::from_low_u64_be(1);
        let prediction_id = H256::from_low_u64_be(100);

        assert_ok!(PredictionMarket::create_market(
            RuntimeOrigin::signed(1),
            market_id,
            prediction_id,
        ));

        let market = Markets::<Test>::get(market_id).unwrap();
        assert_eq!(market.status, MarketStatus::Open);
        assert_eq!(market.total_stake, 0);
        assert_eq!(market.participant_count, 0);
        assert_eq!(market.prediction_id, prediction_id);

        System::assert_last_event(
            Event::<Test>::MarketCreated {
                market_id,
                creator: 1,
                prediction_id,
            }
            .into(),
        );
    });
}

#[test]
fn stake_prediction_reserves_balance_and_updates_market() {
    new_test_ext().execute_with(|| {
        let model_id = H256::from_low_u64_be(1);
        let market_id = H256::from_low_u64_be(2);
        let prediction_id = H256::from_low_u64_be(3);
        let stake_amount: u128 = 1000;

        assert_ok!(PredictionMarket::register_model(
            RuntimeOrigin::signed(1),
            model_id,
            H256::from_low_u64_be(100),
        ));

        assert_ok!(PredictionMarket::create_market(
            RuntimeOrigin::signed(1),
            market_id,
            prediction_id,
        ));

        let free_before = Balances::free_balance(&1);

        assert_ok!(PredictionMarket::stake_prediction(
            RuntimeOrigin::signed(1),
            market_id,
            model_id,
            prediction_id,
            stake_amount,
        ));

        assert_eq!(Balances::reserved_balance(&1), stake_amount as u64);
        assert_eq!(Balances::free_balance(&1), free_before - stake_amount as u64);

        let market = Markets::<Test>::get(market_id).unwrap();
        assert_eq!(market.total_stake, stake_amount);
        assert_eq!(market.participant_count, 1);

        let stake = Stakes::<Test>::get(market_id, model_id).unwrap();
        assert_eq!(stake.model_id, model_id);
        assert_eq!(stake.owner, 1);
        assert_eq!(stake.stake_amount, stake_amount);
        assert!(!stake.settled);
    });
}

#[test]
fn stake_prediction_fails_if_not_model_owner() {
    new_test_ext().execute_with(|| {
        let model_id = H256::from_low_u64_be(1);
        let market_id = H256::from_low_u64_be(2);
        let prediction_id = H256::from_low_u64_be(3);

        assert_ok!(PredictionMarket::register_model(
            RuntimeOrigin::signed(1),
            model_id,
            H256::from_low_u64_be(100),
        ));

        assert_ok!(PredictionMarket::create_market(
            RuntimeOrigin::signed(1),
            market_id,
            prediction_id,
        ));

        assert_noop!(
            PredictionMarket::stake_prediction(
                RuntimeOrigin::signed(2),
                market_id,
                model_id,
                prediction_id,
                1000,
            ),
            Error::<Test>::NotModelOwner
        );
    });
}

#[test]
fn stake_prediction_fails_if_market_not_open() {
    new_test_ext().execute_with(|| {
        let model_id = H256::from_low_u64_be(1);
        let market_id = H256::from_low_u64_be(2);
        let prediction_id = H256::from_low_u64_be(3);

        assert_ok!(PredictionMarket::register_model(
            RuntimeOrigin::signed(1),
            model_id,
            H256::from_low_u64_be(100),
        ));

        assert_ok!(PredictionMarket::create_market(
            RuntimeOrigin::signed(1),
            market_id,
            prediction_id,
        ));

        Markets::<Test>::mutate(market_id, |maybe| {
            if let Some(m) = maybe {
                m.status = MarketStatus::Settled;
            }
        });

        assert_noop!(
            PredictionMarket::stake_prediction(
                RuntimeOrigin::signed(1),
                market_id,
                model_id,
                prediction_id,
                1000,
            ),
            Error::<Test>::MarketNotOpen
        );
    });
}

#[test]
fn stake_prediction_fails_with_insufficient_balance() {
    new_test_ext().execute_with(|| {
        let model_id = H256::from_low_u64_be(1);
        let market_id = H256::from_low_u64_be(2);
        let prediction_id = H256::from_low_u64_be(3);

        assert_ok!(PredictionMarket::register_model(
            RuntimeOrigin::signed(1),
            model_id,
            H256::from_low_u64_be(100),
        ));

        assert_ok!(PredictionMarket::create_market(
            RuntimeOrigin::signed(1),
            market_id,
            prediction_id,
        ));

        assert_noop!(
            PredictionMarket::stake_prediction(
                RuntimeOrigin::signed(1),
                market_id,
                model_id,
                prediction_id,
                100_000,
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn full_settlement_flow() {
    new_test_ext().execute_with(|| {
        let model_id = H256::from_low_u64_be(1);
        let market_id = H256::from_low_u64_be(2);
        let prediction_id = H256::from_low_u64_be(3);
        let market_prediction_id = H256::from_low_u64_be(4);
        let prediction: i128 = 1_500_000;
        let salt = H256::from_low_u64_be(42);
        let model_hash = H256::from_low_u64_be(200);
        let input_hash = H256::from_low_u64_be(300);
        let outcome: i128 = 1_600_000;
        let stake_amount: u128 = 1000;

        // Register model
        assert_ok!(PredictionMarket::register_model(
            RuntimeOrigin::signed(1),
            model_id,
            model_hash,
        ));

        // Submit commitment
        let commitment_hash = make_commitment_hash(prediction, salt, model_hash, input_hash);
        assert_ok!(Verification::submit_commitment(
            RuntimeOrigin::signed(1),
            prediction_id,
            commitment_hash,
        ));

        // Create market
        assert_ok!(PredictionMarket::create_market(
            RuntimeOrigin::signed(1),
            market_id,
            market_prediction_id,
        ));

        // Stake prediction
        assert_ok!(PredictionMarket::stake_prediction(
            RuntimeOrigin::signed(1),
            market_id,
            model_id,
            prediction_id,
            stake_amount,
        ));

        // Reveal prediction
        assert_ok!(Verification::reveal_prediction(
            RuntimeOrigin::signed(1),
            prediction_id,
            prediction,
            salt,
            model_hash,
            input_hash,
        ));

        // Submit ground truth (for market's prediction_id)
        assert_ok!(Verification::submit_ground_truth(
            RuntimeOrigin::signed(1),
            market_prediction_id,
            outcome,
        ));

        // Settle market
        assert_ok!(PredictionMarket::settle_market(
            RuntimeOrigin::signed(1),
            market_id,
        ));

        let market = Markets::<Test>::get(market_id).unwrap();
        assert_eq!(market.status, MarketStatus::Settled);

        let stake = Stakes::<Test>::get(market_id, model_id).unwrap();
        assert!(stake.settled);

        let model = Models::<Test>::get(model_id).unwrap();
        assert_eq!(model.total_predictions, 1);
    });
}

/// Helper: set up a two-model market with different prediction accuracies.
/// Returns (market_id, model_id_a, model_id_b) after settlement.
fn setup_two_model_settlement() {
    let model_id_a = H256::from_low_u64_be(1);
    let model_id_b = H256::from_low_u64_be(2);
    let market_id = H256::from_low_u64_be(10);
    let market_prediction_id = H256::from_low_u64_be(20);
    let prediction_id_a = H256::from_low_u64_be(30);
    let prediction_id_b = H256::from_low_u64_be(31);

    let model_hash_a = H256::from_low_u64_be(100);
    let model_hash_b = H256::from_low_u64_be(101);
    let salt = H256::from_low_u64_be(42);
    let input_hash = H256::from_low_u64_be(300);

    let outcome: i128 = 1_600_000;
    // Model A: perfect prediction
    let prediction_a: i128 = 1_600_000;
    // Model B: off by 800_000_000 -> brier=640B, inverse=360B (below 500B threshold)
    let prediction_b: i128 = 801_600_000;

    // Register models
    assert_ok!(PredictionMarket::register_model(
        RuntimeOrigin::signed(1), model_id_a, model_hash_a,
    ));
    assert_ok!(PredictionMarket::register_model(
        RuntimeOrigin::signed(2), model_id_b, model_hash_b,
    ));

    // Submit commitments
    let hash_a = make_commitment_hash(prediction_a, salt, model_hash_a, input_hash);
    let hash_b = make_commitment_hash(prediction_b, salt, model_hash_b, input_hash);
    assert_ok!(Verification::submit_commitment(
        RuntimeOrigin::signed(1), prediction_id_a, hash_a,
    ));
    assert_ok!(Verification::submit_commitment(
        RuntimeOrigin::signed(2), prediction_id_b, hash_b,
    ));

    // Create market
    assert_ok!(PredictionMarket::create_market(
        RuntimeOrigin::signed(1), market_id, market_prediction_id,
    ));

    // Stake predictions (1000 each)
    assert_ok!(PredictionMarket::stake_prediction(
        RuntimeOrigin::signed(1), market_id, model_id_a, prediction_id_a, 1000,
    ));
    assert_ok!(PredictionMarket::stake_prediction(
        RuntimeOrigin::signed(2), market_id, model_id_b, prediction_id_b, 1000,
    ));

    // Reveal predictions
    assert_ok!(Verification::reveal_prediction(
        RuntimeOrigin::signed(1), prediction_id_a, prediction_a, salt, model_hash_a, input_hash,
    ));
    assert_ok!(Verification::reveal_prediction(
        RuntimeOrigin::signed(2), prediction_id_b, prediction_b, salt, model_hash_b, input_hash,
    ));

    // Submit ground truth
    assert_ok!(Verification::submit_ground_truth(
        RuntimeOrigin::signed(1), market_prediction_id, outcome,
    ));

    // Settle
    assert_ok!(PredictionMarket::settle_market(
        RuntimeOrigin::signed(1), market_id,
    ));
}

#[test]
fn rewards_proportional_to_inverse_brier_score() {
    new_test_ext().execute_with(|| {
        setup_two_model_settlement();

        // Model A (perfect): inverse = 1_000_000_000_000
        // Model B (off 800M): brier = 640_000_000_000, inverse = 360_000_000_000
        // total_inverse = 1_360_000_000_000, pool = 2000
        // reward_A = 2000 * 1_000_000_000_000 / 1_360_000_000_000 = 1470
        // reward_B = 2000 * 360_000_000_000 / 1_360_000_000_000 = 529
        // final_A = 10_000 + 1470 = 11_470
        // final_B = 10_000 + 529 = 10_529

        let balance_a = Balances::free_balance(&1);
        let balance_b = Balances::free_balance(&2);

        assert!(balance_a > balance_b, "closer prediction should earn more");
        assert_eq!(balance_a, 11_470);
        assert_eq!(balance_b, 10_529);
    });
}

#[test]
fn model_reputation_updates_after_settlement() {
    new_test_ext().execute_with(|| {
        let model_id_a = H256::from_low_u64_be(1);
        let model_id_b = H256::from_low_u64_be(2);

        setup_two_model_settlement();

        let model_a = Models::<Test>::get(model_id_a).unwrap();
        let model_b = Models::<Test>::get(model_id_b).unwrap();

        // Model A: inverse=10^12 > 500B threshold -> +100 -> 5100
        assert_eq!(model_a.reputation_score, 5100);
        assert_eq!(model_a.total_predictions, 1);
        assert_eq!(model_a.total_correct, 1);

        // Model B: inverse=360B < 500B threshold -> -200 -> 4800
        assert_eq!(model_b.reputation_score, 4800);
        assert_eq!(model_b.total_predictions, 1);
        assert_eq!(model_b.total_correct, 0);
    });
}
