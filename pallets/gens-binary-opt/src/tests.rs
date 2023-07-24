#![cfg(test)]

use eq_utils::ONE_TOKEN;
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::FixedI64;

use crate::{mock::*, BinaryMode::*};

type Module = crate::Pallet<Test>;
type Error = crate::Error<Test>;

#[test]
fn start_single_binary_option() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            0,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));
    })
}

#[test]
fn start_multiple_similar_binaries() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            0,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));
        assert_err!(
            Module::create(
                RawOrigin::Root.into(),
                BINARY_ID_0,
                0,
                DEPOSIT_OFFSET,
                TARGET_ASSET,
                CallPut(OLD_TARGET_PRICE),
                PROPER_ASSET,
                MINIMAL_DEPOSIT,
                ZERO_FEE,
                PENALTY,
            ),
            Error::AlreadyStarted,
        );
    })
}

#[test]
fn start_multiple_different_binaries() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            0,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_1,
            0,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));
    })
}

#[test]
fn start_with_non_existent_asset() {
    new_test_ext().execute_with(|| {
        assert_err!(
            Module::create(
                RawOrigin::Root.into(),
                BINARY_ID_0,
                0,
                DEPOSIT_OFFSET,
                UNKNOWN_ASSET,
                CallPut(OLD_TARGET_PRICE),
                PROPER_ASSET,
                MINIMAL_DEPOSIT,
                ZERO_FEE,
                PENALTY,
            ),
            eq_assets::Error::<Test>::AssetNotExists,
        );
        assert_err!(
            Module::create(
                RawOrigin::Root.into(),
                BINARY_ID_0,
                0,
                DEPOSIT_OFFSET,
                TARGET_ASSET,
                CallPut(OLD_TARGET_PRICE),
                UNKNOWN_ASSET,
                MINIMAL_DEPOSIT,
                ZERO_FEE,
                PENALTY,
            ),
            eq_assets::Error::<Test>::AssetNotExists,
        );
    })
}

#[test]
fn start_binary_and_end_in_time() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        time_move(60);

        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));
    })
}

#[test]
fn start_binary_and_end_earlier() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        time_move(54);

        assert_err!(
            Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::TryPurgeEarlier,
        );

        time_move(6);

        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));
    })
}

#[test]
fn start_multiple_similar_binaries_consequentially() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        time_move(60);

        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        time_move(40);

        assert_err!(Module::create(
                RawOrigin::Root.into(),
                BINARY_ID_0,
                60,
                DEPOSIT_OFFSET,
                TARGET_ASSET,
                CallPut(OLD_TARGET_PRICE),
                PROPER_ASSET,
                MINIMAL_DEPOSIT,
                ZERO_FEE,
                PENALTY,
            ),
            Error::InvalidId
        );

        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_1,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        time_move(60);

        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_1));
    })
}

#[test]
fn start_binary_option_with_in_out_mode() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            30,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            InOut(TARGET_PRICE_0, TARGET_PRICE_1),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_1,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            InOut(TARGET_PRICE_0, TARGET_PRICE_1),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_2,
            90,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            InOut(TARGET_PRICE_0, TARGET_PRICE_1),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            ONE_TOKEN
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            ONE_TOKEN
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_1,
            true,
            ONE_TOKEN
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_1,
            false,
            ONE_TOKEN
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_2,
            true,
            ONE_TOKEN
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_2,
            false,
            ONE_TOKEN
        ));

        set_target_price(FixedI64::from_inner(0_250_000_000));
        time_move(30);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        set_target_price(FixedI64::from_inner(1_250_000_000));
        time_move(30);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_1));

        set_target_price(FixedI64::from_inner(2_250_000_000));
        time_move(30);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_2));

        assert_ok!(Module::purge(RawOrigin::Signed(USER_3).into(), BINARY_ID_0));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_3).into(), BINARY_ID_1));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_3).into(), BINARY_ID_2));
    })
}

#[test]
fn try_to_end_binary_in_time_but_before_block_finalize() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            7,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        time_move(6);

        assert_err!(
            Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::TryPurgeEarlier,
        );

        // 6 + 1 = 7
        // time has come, waiting for next block
        time_move(1);

        assert_err!(
            Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::TryPurgeEarlier,
        );

        // 7 + 1 = 8
        time_move(1);

        assert_err!(
            Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::TryPurgeEarlier,
        );

        // 8 + 2 = 10
        time_move(2);

        assert_err!(
            Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::TryPurgeEarlier,
        );

        // 10 + 2 = 12
        // new block
        time_move(2);

        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));
    })
}

#[test]
fn binary_results_with_target_price_no_growth() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            4 * ONE_TOKEN,
        ));

        time_move(60);

        assert_err!(
            Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 9 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 11 * ONE_TOKEN);
        assert_eq!(balances[&(get_treasury_account(), PROPER_ASSET)], 0);
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn binary_results_with_target_price_growth() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            4 * ONE_TOKEN,
        ));

        set_target_price(NEW_TARGET_PRICE);
        time_move(60);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 14 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 6 * ONE_TOKEN);
        assert_eq!(balances[&(get_treasury_account(), PROPER_ASSET)], 0);
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn deposit_while_already_participated() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_err!(
            Module::deposit(
                RawOrigin::Signed(USER_0).into(),
                BINARY_ID_0,
                false,
                3 * ONE_TOKEN,
            ),
            Error::DepositForOppositeResult,
        );
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            2 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            1 * ONE_TOKEN,
        ));

        set_target_price(NEW_TARGET_PRICE);
        time_move(60);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 14 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 6 * ONE_TOKEN);
        assert_eq!(balances[&(get_treasury_account(), PROPER_ASSET)], 0);
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn binary_results_with_multiple_winners_one_loser() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            4 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            true,
            3 * ONE_TOKEN,
        ));

        set_target_price(NEW_TARGET_PRICE);
        time_move(60);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::claim(RawOrigin::Signed(USER_2).into(), BINARY_ID_0));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 11 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 6 * ONE_TOKEN);
        assert_eq!(balances[&(USER_2, PROPER_ASSET)], 13 * ONE_TOKEN);
        assert_eq!(balances[&(get_treasury_account(), PROPER_ASSET)], 0);
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn try_to_deposit_when_the_participation_time_is_over() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            4 * ONE_TOKEN,
        ));

        time_move(54);

        assert_err!(
            Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0),
            Error::TryClaimEarlier,
        );
        assert_err!(
            Module::deposit(
                RawOrigin::Signed(USER_2).into(),
                BINARY_ID_0,
                true,
                3 * ONE_TOKEN,
            ),
            Error::ParticipateTimeIsOver,
        );

        set_target_price(NEW_TARGET_PRICE);
        time_move(12);

        assert_err!(
            Module::deposit(
                RawOrigin::Signed(USER_2).into(),
                BINARY_ID_0,
                true,
                3 * ONE_TOKEN,
            ),
            Error::ParticipateTimeIsOver,
        );

        assert_ok!(Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        assert_err!(
            Module::deposit(
                RawOrigin::Signed(USER_2).into(),
                BINARY_ID_0,
                true,
                3 * ONE_TOKEN,
            ),
            Error::NoBinary,
        );

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 14 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 6 * ONE_TOKEN);
        assert_eq!(balances[&(USER_2, PROPER_ASSET)], 10 * ONE_TOKEN);
        assert_eq!(balances[&(get_treasury_account(), PROPER_ASSET)], 0);
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn try_to_deposit_less_than_minimal_deposit() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_err!(
            Module::deposit(RawOrigin::Signed(USER_0).into(), BINARY_ID_0, true, 1),
            Error::LowDeposit,
        );
    })
}

#[test]
fn try_to_withdraw_without_deposit() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            4 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            true,
            3 * ONE_TOKEN,
        ));

        assert_ok!(Module::withdraw(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
        ));
        assert_err!(
            Module::withdraw(RawOrigin::Signed(USER_2).into(), BINARY_ID_0),
            Error::NoDeposit,
        );

        set_target_price(NEW_TARGET_PRICE);
        time_move(60);

        assert_err!(
            Module::withdraw(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::ParticipateTimeIsOver,
        );

        assert_ok!(Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_2).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::purge(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));

        assert_err!(
            Module::deposit(
                RawOrigin::Signed(USER_2).into(),
                BINARY_ID_0,
                true,
                3 * ONE_TOKEN,
            ),
            Error::NoBinary,
        );

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 14 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 6 * ONE_TOKEN);
        assert_eq!(
            balances[&(USER_2, PROPER_ASSET)],
            9_85 * ONE_HUNDREDTH_TOKEN
        );
        assert_eq!(
            balances[&(get_treasury_account(), PROPER_ASSET)],
            0_15 * ONE_HUNDREDTH_TOKEN
        );
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn binary_results_with_no_winners() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            true,
            4 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            true,
            3 * ONE_TOKEN,
        ));

        time_move(60);

        assert_err!(
            Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_2).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 9 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 6 * ONE_TOKEN);
        assert_eq!(balances[&(USER_2, PROPER_ASSET)], 7 * ONE_TOKEN);
        assert_eq!(
            balances[&(get_treasury_account(), PROPER_ASSET)],
            8 * ONE_TOKEN
        );
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn binary_results_with_all_winners() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            false,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            4 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            false,
            3 * ONE_TOKEN,
        ));

        time_move(60);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));
        assert_ok!(Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));
        assert_ok!(Module::claim(RawOrigin::Signed(USER_2).into(), BINARY_ID_0));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 10 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 10 * ONE_TOKEN);
        assert_eq!(balances[&(USER_2, PROPER_ASSET)], 10 * ONE_TOKEN);
        assert_eq!(balances[&(get_treasury_account(), PROPER_ASSET)], 0);
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn binary_results_with_multiple_winners_multiple_losers() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            false,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            true,
            2 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            false,
            3 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_3).into(),
            BINARY_ID_0,
            true,
            4 * ONE_TOKEN,
        ));

        time_move(60);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0,),
            Error::NoReward,
        );
        assert_ok!(Module::claim(RawOrigin::Signed(USER_2).into(), BINARY_ID_0));
        assert_err!(
            Module::claim(RawOrigin::Signed(USER_3).into(), BINARY_ID_0,),
            Error::NoReward,
        );
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 11_5 * ONE_TENTH_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 8 * ONE_TOKEN);
        assert_eq!(balances[&(USER_2, PROPER_ASSET)], 14_5 * ONE_TENTH_TOKEN);
        assert_eq!(balances[&(USER_3, PROPER_ASSET)], 6 * ONE_TOKEN);
        assert_eq!(balances[&(get_treasury_account(), PROPER_ASSET)], 0);
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn claim_for_other() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            false,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            true,
            2 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            false,
            3 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_3).into(),
            BINARY_ID_0,
            true,
            4 * ONE_TOKEN,
        ));

        time_move(60);

        assert_ok!(Module::claim_other(
            RawOrigin::Signed(USER_0).into(),
            USER_0,
            BINARY_ID_0,
        ));
        assert_err!(
            Module::claim_other(RawOrigin::Signed(USER_0).into(), USER_1, BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::claim_other(
            RawOrigin::Signed(USER_0).into(),
            USER_2,
            BINARY_ID_0,
        ));
        assert_err!(
            Module::claim_other(RawOrigin::Signed(USER_0).into(), USER_3, BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 11_5 * ONE_TENTH_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 8 * ONE_TOKEN);
        assert_eq!(balances[&(USER_2, PROPER_ASSET)], 14_5 * ONE_TENTH_TOKEN);
        assert_eq!(balances[&(USER_3, PROPER_ASSET)], 6 * ONE_TOKEN);
        assert_eq!(balances[&(get_treasury_account(), PROPER_ASSET)], 0);
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn rounding_error_residue_transfer_to_user_0() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            false,
            2 * ONE_TOKEN,
        ));

        // true coefficient is 4
        // false coefficient is 4/3
        time_move(60);

        assert_err!(
            Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));
        assert_ok!(Module::claim(RawOrigin::Signed(USER_2).into(), BINARY_ID_0));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 9 * ONE_TOKEN);
        assert_eq!(
            balances[&(USER_1, PROPER_ASSET)],
            10 * ONE_TOKEN + ONE_THIRD_TOKEN
        );
        assert_eq!(
            balances[&(USER_2, PROPER_ASSET)],
            10 * ONE_TOKEN + 2 * ONE_THIRD_TOKEN + 1
        );
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn rounding_error_residue_transfer_to_user_1() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            false,
            2 * ONE_TOKEN,
        ));

        // true coefficient is 4
        // false coefficient is 4/3
        time_move(60);

        assert_err!(
            Module::claim(RawOrigin::Signed(USER_0).into(), BINARY_ID_0),
            Error::NoReward,
        );
        assert_ok!(Module::claim(RawOrigin::Signed(USER_2).into(), BINARY_ID_0));
        assert_ok!(Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 9 * ONE_TOKEN);
        assert_eq!(
            balances[&(USER_1, PROPER_ASSET)],
            10 * ONE_TOKEN + ONE_THIRD_TOKEN + 1
        );
        assert_eq!(
            balances[&(USER_2, PROPER_ASSET)],
            10 * ONE_TOKEN + 2 * ONE_THIRD_TOKEN
        );
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn try_purge_with_winners() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            ZERO_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            1 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            true,
            3 * ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_2).into(),
            BINARY_ID_0,
            false,
            4 * ONE_TOKEN,
        ));
        assert_ok!(Module::withdraw(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
        ));

        time_move(60);

        assert_err!(
            Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0),
            Error::TryPurgeWithWinners,
        );
        assert_ok!(Module::claim(RawOrigin::Signed(USER_2).into(), BINARY_ID_0));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));

        let balances = get_balances();

        assert_eq!(
            balances[&(USER_0, PROPER_ASSET)],
            9_95 * ONE_HUNDREDTH_TOKEN
        );
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 7 * ONE_TOKEN);
        assert_eq!(balances[&(USER_2, PROPER_ASSET)], 13 * ONE_TOKEN);
        assert_eq!(
            balances[&(get_treasury_account(), PROPER_ASSET)],
            0_05 * ONE_HUNDREDTH_TOKEN
        );
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

#[test]
fn fee_test() {
    new_test_ext().execute_with(|| {
        assert_ok!(Module::create(
            RawOrigin::Root.into(),
            BINARY_ID_0,
            60,
            DEPOSIT_OFFSET,
            TARGET_ASSET,
            CallPut(OLD_TARGET_PRICE),
            PROPER_ASSET,
            MINIMAL_DEPOSIT,
            TEN_PERCENT_FEE,
            PENALTY,
        ));

        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_0).into(),
            BINARY_ID_0,
            true,
            ONE_TOKEN,
        ));
        assert_ok!(Module::deposit(
            RawOrigin::Signed(USER_1).into(),
            BINARY_ID_0,
            false,
            ONE_TOKEN,
        ));

        time_move(60);

        assert_ok!(Module::claim(RawOrigin::Signed(USER_1).into(), BINARY_ID_0));
        assert_ok!(Module::purge(RawOrigin::Signed(USER_0).into(), BINARY_ID_0));

        let balances = get_balances();
        // won: 1 TOKEN
        // fee: 10% * 1 ONE_TOKEN == 0.1 ONE_TOKEN == 1 ONE_TENTH_TOKEN
        assert_eq!(balances[&(USER_0, PROPER_ASSET)], 9 * ONE_TOKEN);
        assert_eq!(balances[&(USER_1, PROPER_ASSET)], 109 * ONE_TENTH_TOKEN);
        assert_eq!(
            balances[&(get_treasury_account(), PROPER_ASSET)],
            ONE_TENTH_TOKEN
        );
        assert_eq!(balances[&(get_pallet_account(), PROPER_ASSET)], 0);
    })
}

