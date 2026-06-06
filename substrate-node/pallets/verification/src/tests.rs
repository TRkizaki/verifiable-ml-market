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
fn submit_commitment_stores_commitment() {
    new_test_ext().execute_with(|| {
        let prediction_id = H256::from_low_u64_be(1);
        let commitment_hash = H256::from_low_u64_be(100);

        assert_ok!(Verification::submit_commitment(
            RuntimeOrigin::signed(1),
            prediction_id,
            commitment_hash,
        ));

        let commitment = Commitments::<Test>::get(prediction_id).unwrap();
        assert_eq!(commitment.submitter, 1);
        assert_eq!(commitment.commitment_hash, commitment_hash);
        assert_eq!(commitment.revealed, false);

        System::assert_last_event(
            Event::<Test>::CommitmentSubmitted {
                prediction_id,
                submitter: 1,
            }
            .into(),
        );
    });
}

#[test]
fn submit_commitment_rejects_duplicate() {
    new_test_ext().execute_with(|| {
        let prediction_id = H256::from_low_u64_be(1);
        let commitment_hash = H256::from_low_u64_be(100);

        assert_ok!(Verification::submit_commitment(
            RuntimeOrigin::signed(1),
            prediction_id,
            commitment_hash,
        ));

        assert_noop!(
            Verification::submit_commitment(
                RuntimeOrigin::signed(2),
                prediction_id,
                commitment_hash,
            ),
            Error::<Test>::CommitmentAlreadyExists
        );
    });
}

#[test]
fn reveal_prediction_succeeds_with_matching_hash() {
    new_test_ext().execute_with(|| {
        let prediction_id = H256::from_low_u64_be(1);
        let prediction: i128 = 1_500_000;
        let salt = H256::from_low_u64_be(42);
        let model_hash = H256::from_low_u64_be(200);
        let input_hash = H256::from_low_u64_be(300);

        let commitment_hash = make_commitment_hash(prediction, salt, model_hash, input_hash);

        assert_ok!(Verification::submit_commitment(
            RuntimeOrigin::signed(1),
            prediction_id,
            commitment_hash,
        ));

        assert_ok!(Verification::reveal_prediction(
            RuntimeOrigin::signed(1),
            prediction_id,
            prediction,
            salt,
            model_hash,
            input_hash,
        ));

        let commitment = Commitments::<Test>::get(prediction_id).unwrap();
        assert!(commitment.revealed);

        let reveal = Reveals::<Test>::get(prediction_id).unwrap();
        assert_eq!(reveal.prediction, prediction);
        assert_eq!(reveal.model_hash, model_hash);
        assert_eq!(reveal.input_hash, input_hash);
        assert_eq!(reveal.submitter, 1);

        System::assert_last_event(
            Event::<Test>::PredictionRevealed {
                prediction_id,
                submitter: 1,
                prediction,
            }
            .into(),
        );
    });
}

#[test]
fn reveal_prediction_fails_on_hash_mismatch() {
    new_test_ext().execute_with(|| {
        let prediction_id = H256::from_low_u64_be(1);
        let commitment_hash = H256::from_low_u64_be(999);

        assert_ok!(Verification::submit_commitment(
            RuntimeOrigin::signed(1),
            prediction_id,
            commitment_hash,
        ));

        assert_noop!(
            Verification::reveal_prediction(
                RuntimeOrigin::signed(1),
                prediction_id,
                1_500_000,
                H256::from_low_u64_be(42),
                H256::from_low_u64_be(200),
                H256::from_low_u64_be(300),
            ),
            Error::<Test>::VerificationMismatch
        );

        assert!(Reveals::<Test>::get(prediction_id).is_none());
    });
}

#[test]
fn reveal_prediction_fails_if_not_commitment_owner() {
    new_test_ext().execute_with(|| {
        let prediction_id = H256::from_low_u64_be(1);
        let prediction: i128 = 1_500_000;
        let salt = H256::from_low_u64_be(42);
        let model_hash = H256::from_low_u64_be(200);
        let input_hash = H256::from_low_u64_be(300);

        let commitment_hash = make_commitment_hash(prediction, salt, model_hash, input_hash);

        assert_ok!(Verification::submit_commitment(
            RuntimeOrigin::signed(1),
            prediction_id,
            commitment_hash,
        ));

        assert_noop!(
            Verification::reveal_prediction(
                RuntimeOrigin::signed(2),
                prediction_id,
                prediction,
                salt,
                model_hash,
                input_hash,
            ),
            Error::<Test>::NotCommitmentOwner
        );
    });
}

#[test]
fn double_reveal_rejected() {
    new_test_ext().execute_with(|| {
        let prediction_id = H256::from_low_u64_be(1);
        let prediction: i128 = 1_500_000;
        let salt = H256::from_low_u64_be(42);
        let model_hash = H256::from_low_u64_be(200);
        let input_hash = H256::from_low_u64_be(300);

        let commitment_hash = make_commitment_hash(prediction, salt, model_hash, input_hash);

        assert_ok!(Verification::submit_commitment(
            RuntimeOrigin::signed(1),
            prediction_id,
            commitment_hash,
        ));

        assert_ok!(Verification::reveal_prediction(
            RuntimeOrigin::signed(1),
            prediction_id,
            prediction,
            salt,
            model_hash,
            input_hash,
        ));

        assert_noop!(
            Verification::reveal_prediction(
                RuntimeOrigin::signed(1),
                prediction_id,
                prediction,
                salt,
                model_hash,
                input_hash,
            ),
            Error::<Test>::AlreadyRevealed
        );
    });
}

#[test]
fn submit_ground_truth_stores_correctly() {
    new_test_ext().execute_with(|| {
        let prediction_id = H256::from_low_u64_be(1);
        let outcome: i128 = 1_600_000;

        assert_ok!(Verification::submit_ground_truth(
            RuntimeOrigin::signed(1),
            prediction_id,
            outcome,
        ));

        let truth = GroundTruths::<Test>::get(prediction_id).unwrap();
        assert_eq!(truth.outcome, outcome);
        assert_eq!(truth.submitter, 1);

        System::assert_last_event(
            Event::<Test>::GroundTruthSubmitted {
                prediction_id,
                outcome,
            }
            .into(),
        );
    });
}

#[test]
fn duplicate_ground_truth_rejected() {
    new_test_ext().execute_with(|| {
        let prediction_id = H256::from_low_u64_be(1);

        assert_ok!(Verification::submit_ground_truth(
            RuntimeOrigin::signed(1),
            prediction_id,
            1_600_000,
        ));

        assert_noop!(
            Verification::submit_ground_truth(
                RuntimeOrigin::signed(2),
                prediction_id,
                1_700_000,
            ),
            Error::<Test>::GroundTruthAlreadySubmitted
        );
    });
}
