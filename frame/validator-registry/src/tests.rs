use crate::{Error, mock::*};
use frame_support::{assert_ok, assert_noop};

#[test]
fn registration_and_unregistration_should_work() {
	new_test_ext().execute_with(|| {
        assert_eq!(ValidatorRegistry::mission_of(1), 0);
        assert_ok!(ValidatorRegistry::register(Origin::signed(1), 10));
        assert_eq!(ValidatorRegistry::mission_of(1), 10);
        assert_ok!(ValidatorRegistry::unregister(Origin::signed(1)));
        assert_eq!(ValidatorRegistry::mission_of(1), 0);
	});
}

#[test]
fn re_registration_should_not_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(ValidatorRegistry::register(Origin::signed(1), 10));
        assert_eq!(ValidatorRegistry::mission_of(1), 10);
        assert_noop!(
            ValidatorRegistry::register(Origin::signed(1), 11),
            Error::<Test>::AlreadyRegistered
        );
    });
}

#[test]
fn registration_with_invalid_mission_id_should_not_work() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ValidatorRegistry::register(Origin::signed(1), 0),
            Error::<Test>::InvalidMissionId
        );
        assert_noop!(
            ValidatorRegistry::register(Origin::signed(1), 13),
            Error::<Test>::InvalidMissionId
        );
    });
}
