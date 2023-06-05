// This file is part of Equilibrium.

// Copyright (C) 2023 EQ Lab.
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]

use crate::{mock::*, EpochInfo, PendingWithdrawal, MINIMAL_DURATION};

use eq_primitives::{
    asset,
    balance::BalanceGetter,
    balance_number::EqFixedU128,
    subaccount::{SubAccType, SubaccountsManager},
    OrderSide, OrderType, SignedBalance,
};
use frame_benchmarking::Zero as _;
use frame_support::{assert_err, assert_ok};
use sp_runtime::{FixedI64, Perbill};

pub type Error = crate::Error<Test>;

#[test]
fn create_new_pool_ok() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));

        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();

        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 0,
                    total_deposit: 0,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            )]
        );
    });
}

#[test]
fn create_new_pool_already_added() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));

        assert_err!(
            MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 300),
            Error::PoolAlreadyExists
        );

        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();

        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 0,
                    total_deposit: 0,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            )]
        );
    });
}

#[test]
fn change_min_amount_ok() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));

        assert_ok!(MmPool::change_min_amount(
            RuntimeOrigin::root(),
            asset::BTC,
            200
        ));

        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();

        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 200,
                    total_staked: 0,
                    total_deposit: 0,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            )]
        );
    });
}

#[test]
fn change_min_amount_no_pool() {
    new_test_ext().execute_with(|| {
        assert_err!(
            MmPool::change_min_amount(RuntimeOrigin::root(), asset::BTC, 300),
            Error::NoPoolWithCurrency
        );

        assert_eq!(MmPool::pools(), vec![]);
    });
}

#[test]
fn epoch_counter() {
    new_test_ext().execute_with(|| {
        TimeMock::set_secs(0);
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 0,
                started_at: 0,
                duration: 100,
                new_duration: None
            }
        );

        // Time = 499
        // Block = 83
        TimeMock::move_secs(499); // Not 5th epoch yet
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 4,
                started_at: 400,
                duration: 100,
                new_duration: None
            }
        );

        // Time = 500
        // Block = 83
        TimeMock::move_secs(1); // Not new block yet
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 4,
                started_at: 400,
                duration: 100,
                new_duration: None
            }
        );

        // Time = 504
        // Block = 84
        TimeMock::move_secs(4); // New block
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 5,
                started_at: 500,
                duration: 100,
                new_duration: None
            }
        );
    });
}

#[test]
fn epoch_set_next() {
    new_test_ext().execute_with(|| {
        TimeMock::set_secs(0);
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 0,
                started_at: 0,
                duration: 100,
                new_duration: None
            }
        );

        TimeMock::move_secs(300);
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 3,
                started_at: 300,
                duration: 100,
                new_duration: None
            }
        );

        assert_err!(
            MmPool::set_epoch_duration(RuntimeOrigin::root(), 200),
            Error::WrongNewDuration
        );
        assert_err!(
            MmPool::set_epoch_duration(RuntimeOrigin::root(), 0),
            Error::WrongNewDuration
        );
        assert_err!(
            MmPool::set_epoch_duration(RuntimeOrigin::root(), MINIMAL_DURATION - 1),
            Error::WrongNewDuration
        );

        assert_ok!(MmPool::set_epoch_duration(
            RuntimeOrigin::root(),
            MINIMAL_DURATION
        ));
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 3,
                started_at: 300,
                duration: 100,
                new_duration: Some(MINIMAL_DURATION)
            }
        );

        TimeMock::move_secs(100);
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 4,
                started_at: 400,
                duration: MINIMAL_DURATION,
                new_duration: None
            }
        );

        TimeMock::move_secs(100); // Epoch is now MINIMAL_DURATION
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 4,
                started_at: 400,
                duration: MINIMAL_DURATION,
                new_duration: None
            }
        );

        TimeMock::move_secs(MINIMAL_DURATION);
        assert_eq!(
            MmPool::epoch(),
            EpochInfo {
                counter: 5,
                started_at: MINIMAL_DURATION + 400,
                duration: MINIMAL_DURATION,
                new_duration: None
            }
        );
    });
}

#[test]
fn epoch_advance() {
    new_test_ext().execute_with(|| {
        TimeMock::set_secs(0);
        // Epoch 0

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 1));
        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 1000, asset::BTC));

        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();
        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 1,
                    total_staked: 1000,
                    total_deposit: 1000,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            )]
        );

        assert_ok!(MmPool::request_withdrawal(
            RuntimeOrigin::signed(1),
            500,
            asset::BTC
        ));
        assert_eq!(
            MmPool::pools()[0].1.total_pending_withdrawals,
            crate::PendingWithdrawal {
                last_epoch: 0,
                available: 0,
                available_next_epoch: 0,
                requested: 500,
            }
        );

        TimeMock::move_secs(100);
        // Epoch 1
        assert_ok!(MmPool::request_withdrawal(
            RuntimeOrigin::signed(1),
            55,
            asset::BTC
        ));
        assert_eq!(
            MmPool::pools()[0].1.total_pending_withdrawals,
            crate::PendingWithdrawal {
                last_epoch: 1,
                available: 0,
                available_next_epoch: 500,
                requested: 55,
            }
        );

        TimeMock::move_secs(100);
        // Epoch 2
        assert_eq!(
            MmPool::pools()[0].1.total_pending_withdrawals,
            crate::PendingWithdrawal {
                last_epoch: 2,
                available: 500,
                available_next_epoch: 55,
                requested: 0,
            }
        );

        TimeMock::move_secs(100);
        // Epoch 3
        assert_eq!(
            MmPool::pools()[0].1.total_pending_withdrawals,
            crate::PendingWithdrawal {
                last_epoch: 3,
                available: 555,
                available_next_epoch: 0,
                requested: 0,
            }
        );

        TimeMock::move_secs(100);
        // Epoch 4
        assert_eq!(
            MmPool::pools()[0].1.total_pending_withdrawals,
            crate::PendingWithdrawal {
                last_epoch: 4,
                available: 555,
                available_next_epoch: 0,
                requested: 0,
            }
        );
        assert_ok!(MmPool::withdraw(RuntimeOrigin::signed(1), asset::BTC));
        assert_eq!(
            MmPool::pools()[0].1.total_pending_withdrawals,
            crate::PendingWithdrawal {
                last_epoch: 4,
                available: 0,
                available_next_epoch: 0,
                requested: 0,
            }
        );
    });
}

#[test]
fn pending_withdrawal() {
    const PW: PendingWithdrawal<u64> = PendingWithdrawal {
        last_epoch: 100,
        available: 1,
        available_next_epoch: 2,
        requested: 3,
    };

    let mut pw0 = PW;
    pw0.advance_epoch(100);
    assert_eq!(
        pw0,
        PendingWithdrawal {
            last_epoch: 100,
            available: 1,
            available_next_epoch: 2,
            requested: 3,
        }
    );

    let mut pw1 = PW;
    pw1.advance_epoch(101);
    assert_eq!(
        pw1,
        PendingWithdrawal {
            last_epoch: 101,
            available: 3,
            available_next_epoch: 3,
            requested: 0,
        }
    );

    let mut pw2 = PW;
    pw2.advance_epoch(102);
    assert_eq!(
        pw2,
        PendingWithdrawal {
            last_epoch: 102,
            available: 6,
            available_next_epoch: 0,
            requested: 0,
        }
    );

    let mut pw3 = PW;
    pw3.advance_epoch(103);
    assert_eq!(
        pw3,
        PendingWithdrawal {
            last_epoch: 103,
            available: 6,
            available_next_epoch: 0,
            requested: 0,
        }
    );
}

#[test]
fn deposit_multiple_pools() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 10));

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::ETH, 300));

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 100, asset::BTC,));

        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();
        let eth_account_id = MmPool::generate_pool_acc(asset::ETH).unwrap();

        assert_eq!(
            MmPool::pools(),
            vec![
                (
                    asset::BTC,
                    crate::MmPoolInfo {
                        account_id: btc_account_id,
                        min_amount: 10,
                        total_staked: 100,
                        total_deposit: 100,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
                (
                    asset::ETH,
                    crate::MmPoolInfo {
                        account_id: eth_account_id,
                        min_amount: 300,
                        total_staked: 0,
                        total_deposit: 0,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
            ]
        );

        assert_eq!(
            MmPool::deposits(1),
            vec![(
                asset::BTC,
                crate::LenderInfo {
                    deposit: 100,
                    pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    }
                }
            ),]
        );

        assert_eq!(
            EqBalances::get_balance(&btc_account_id, &asset::BTC),
            SignedBalance::Positive(100)
        );

        // one more deposit with the same currency

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 10, asset::BTC,));

        assert_eq!(
            MmPool::pools(),
            vec![
                (
                    asset::BTC,
                    crate::MmPoolInfo {
                        account_id: btc_account_id,
                        min_amount: 10,
                        total_staked: 110,
                        total_deposit: 110,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
                (
                    asset::ETH,
                    crate::MmPoolInfo {
                        account_id: eth_account_id,
                        min_amount: 300,
                        total_staked: 0,
                        total_deposit: 0,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
            ]
        );

        assert_eq!(
            MmPool::deposits(1),
            vec![(
                asset::BTC,
                crate::LenderInfo {
                    deposit: 110,
                    pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    }
                }
            ),]
        );

        assert_eq!(
            EqBalances::get_balance(&btc_account_id, &asset::BTC),
            SignedBalance::Positive(110)
        );

        // one more deposit with the another currency

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 333, asset::ETH,));

        assert_eq!(
            MmPool::pools(),
            vec![
                (
                    asset::BTC,
                    crate::MmPoolInfo {
                        account_id: btc_account_id,
                        min_amount: 10,
                        total_staked: 110,
                        total_deposit: 110,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
                (
                    asset::ETH,
                    crate::MmPoolInfo {
                        account_id: eth_account_id,
                        min_amount: 300,
                        total_staked: 333,
                        total_deposit: 333,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
            ]
        );

        assert_eq!(
            MmPool::deposits(1),
            vec![
                (
                    asset::BTC,
                    crate::LenderInfo {
                        deposit: 110,
                        pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        }
                    }
                ),
                (
                    asset::ETH,
                    crate::LenderInfo {
                        deposit: 333,
                        pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        }
                    }
                ),
            ]
        );

        assert_eq!(
            EqBalances::get_balance(&eth_account_id, &asset::ETH),
            SignedBalance::Positive(333)
        );
    });
}

#[test]
fn deposit_single_pool() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::ETH, 300));

        assert_err!(
            MmPool::deposit(RuntimeOrigin::signed(1), 100, asset::BTC,),
            Error::NoPoolWithCurrency
        );

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));

        assert_err!(
            MmPool::deposit(RuntimeOrigin::signed(1), 10, asset::BTC,),
            Error::AmountLessThanMin
        );

        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();
        let eth_account_id = MmPool::generate_pool_acc(asset::ETH).unwrap();

        assert_eq!(
            MmPool::pools(),
            vec![
                (
                    asset::BTC,
                    crate::MmPoolInfo {
                        account_id: btc_account_id,
                        min_amount: 100,
                        total_staked: 0,
                        total_deposit: 0,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
                (
                    asset::ETH,
                    crate::MmPoolInfo {
                        account_id: eth_account_id,
                        min_amount: 300,
                        total_staked: 0,
                        total_deposit: 0,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
            ]
        );

        assert_eq!(MmPool::deposits(1), vec![]);

        assert_eq!(
            EqBalances::get_balance(&btc_account_id, &asset::BTC),
            SignedBalance::zero()
        );

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 300, asset::BTC,));

        assert_eq!(
            MmPool::pools(),
            vec![
                (
                    asset::BTC,
                    crate::MmPoolInfo {
                        account_id: btc_account_id,
                        min_amount: 100,
                        total_staked: 300,
                        total_deposit: 300,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
                (
                    asset::ETH,
                    crate::MmPoolInfo {
                        account_id: eth_account_id,
                        min_amount: 300,
                        total_staked: 0,
                        total_deposit: 0,
                        total_borrowed: 0,
                        total_pending_withdrawals: crate::PendingWithdrawal {
                            last_epoch: 0,
                            available: 0,
                            available_next_epoch: 0,
                            requested: 0
                        },
                    }
                ),
            ]
        );

        assert_eq!(
            MmPool::deposits(1),
            vec![(
                asset::BTC,
                crate::LenderInfo {
                    deposit: 300,
                    pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    }
                }
            ),]
        );

        assert_eq!(
            EqBalances::get_balance(&btc_account_id, &asset::BTC),
            SignedBalance::Positive(300)
        );
    });
}

#[test]
fn withdraw_request_ok() {
    new_test_ext().execute_with(|| {
        TimeMock::set_secs(0);

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();
        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 200,
                    total_deposit: 200,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            ),]
        );
        assert_eq!(
            MmPool::deposits(1),
            vec![(
                asset::BTC,
                crate::LenderInfo {
                    deposit: 200,
                    pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    }
                }
            ),]
        );

        assert_ok!(MmPool::request_withdrawal(
            RuntimeOrigin::signed(1),
            150,
            asset::BTC
        ));

        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 200,
                    total_deposit: 200,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 150
                    },
                }
            ),]
        );
        assert_eq!(
            MmPool::deposits(1),
            vec![(
                asset::BTC,
                crate::LenderInfo {
                    deposit: 200,
                    pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 150
                    }
                }
            ),]
        );

        TimeMock::move_secs(201); // two epochs passed
        assert_eq!(MmPool::epoch().counter, 2);

        assert_ok!(MmPool::withdraw(RuntimeOrigin::signed(1), asset::BTC));
        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 50,
                    total_deposit: 50,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 2,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            )]
        );
        assert_eq!(
            MmPool::deposits(1),
            vec![(
                asset::BTC,
                crate::LenderInfo {
                    deposit: 50,
                    pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 2,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    }
                }
            ),]
        );
        assert_eq!(
            EqBalances::get_balance(&1, &asset::BTC),
            SignedBalance::Positive(950)
        )
    });
}

#[test]
fn withdraw_no_deposit() {
    new_test_ext().execute_with(|| {
        TimeMock::set_secs(0);

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::ETH, 100));
        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        assert_err!(
            MmPool::request_withdrawal(RuntimeOrigin::signed(2), 150, asset::BTC),
            Error::NoDeposit,
        );
        assert_err!(
            MmPool::request_withdrawal(RuntimeOrigin::signed(1), 150, asset::ETH),
            Error::NoDeposit,
        );
    });
}

#[test]
fn withdraw_no_request() {
    new_test_ext().execute_with(|| {
        TimeMock::set_secs(0);

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        assert_eq!(
            EqBalances::get_balance(&1, &asset::BTC),
            SignedBalance::Positive(800),
        );

        assert_ok!(MmPool::request_withdrawal(
            RuntimeOrigin::signed(1),
            150,
            asset::BTC
        ));
        // Too early
        assert_err!(
            MmPool::withdraw(RuntimeOrigin::signed(1), asset::BTC),
            Error::WithdrawalNotRequested
        );

        TimeMock::move_secs(200);
        assert_ok!(MmPool::withdraw(RuntimeOrigin::signed(1), asset::BTC));
        assert_eq!(
            EqBalances::get_balance(&1, &asset::BTC),
            SignedBalance::Positive(950),
        );
        // Already withdrawed
        assert_err!(
            MmPool::withdraw(RuntimeOrigin::signed(1), asset::BTC),
            Error::WithdrawalNotRequested
        );
    });
}

#[test]
fn withdraw_not_enough() {
    new_test_ext().execute_with(|| {
        TimeMock::set_secs(0);

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        assert_err!(
            MmPool::request_withdrawal(RuntimeOrigin::signed(1), 250, asset::BTC),
            Error::NotEnoughToWithdraw,
        );
        assert_ok!(MmPool::request_withdrawal(
            RuntimeOrigin::signed(1),
            100,
            asset::BTC
        ));
        assert_err!(
            MmPool::request_withdrawal(RuntimeOrigin::signed(1), 101, asset::BTC),
            Error::NotEnoughToWithdraw,
        );
    })
}

#[test]
fn market_maker_create_allocation_ok() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::set_allocations(
            RuntimeOrigin::root(),
            MM_ID[0],
            vec![
                (asset::ETH, Perbill::from_percent(20)),
                (asset::BTC, Perbill::from_percent(30)),
            ]
        ));

        assert_eq!(
            MmPool::mm(MM_ID[0]),
            vec![
                (
                    asset::BTC,
                    crate::MmInfo {
                        weight: Perbill::from_percent(30),
                        borrowed: 0
                    }
                ),
                (
                    asset::ETH,
                    crate::MmInfo {
                        weight: Perbill::from_percent(20),
                        borrowed: 0
                    }
                ),
            ]
        )
    });
}

#[test]
fn market_maker_create_allocation_unknown_asset() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_err!(
            MmPool::set_allocations(
                RuntimeOrigin::root(),
                MM_ID[0],
                vec![
                    (asset::ETH, Perbill::from_percent(20)),
                    (asset::BTC, Perbill::from_percent(30)),
                    (asset::XDOT, Perbill::from_percent(100)),
                ]
            ),
            eq_assets::Error::<Test>::AssetNotExists
        );
    });
}

#[test]
fn borrower_add_ok() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::add_manager(
            RuntimeOrigin::root(),
            TRADER_EXIST,
            MM_ID[0]
        ));
        let trading_acc = MmPool::generate_trade_acc(MM_ID[0], &TRADER_EXIST).unwrap();
        assert_eq!(
            MmPool::managers(TRADER_EXIST),
            Some((MM_ID[0], trading_acc))
        );
    });
}

#[test]
fn borrower_add_existing() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::add_manager(
            RuntimeOrigin::root(),
            TRADER_EXIST,
            MM_ID[0]
        ));
        assert_err!(
            MmPool::add_manager(RuntimeOrigin::root(), TRADER_EXIST, MM_ID[1]),
            Error::BorrowerAlreadyExists,
        );
    });
}

// #[test]
// fn borrower_remove() {
//     new_test_ext().execute_with(|| {
//
//     });
// }

#[test]
fn borrow_ok() {
    new_test_ext().execute_with(|| {
        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::set_allocations(
            RuntimeOrigin::root(),
            MM_ID[0],
            vec![(asset::BTC, Perbill::from_percent(100))]
        ));
        assert_ok!(MmPool::add_manager(
            RuntimeOrigin::root(),
            TRADER_EXIST,
            MM_ID[0]
        ));
        let trading_acc = MmPool::generate_trade_acc(MM_ID[0], &TRADER_EXIST).unwrap();
        let borrower_subacc =
            SubaccountsManagerMock::get_subaccount_id(&TRADER_EXIST, &SubAccType::Trader).unwrap();
        let trading_subacc =
            SubaccountsManagerMock::get_subaccount_id(&trading_acc, &SubAccType::Trader).unwrap();

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 200,
                    total_deposit: 200,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            ),]
        );

        assert_err!(
            MmPool::borrow(RuntimeOrigin::signed(TRADER_NOT_EXIST), 100, asset::BTC),
            Error::BorrowerDoesNotExist,
        );
        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            100,
            asset::BTC
        ));

        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 200,
                    total_deposit: 100,
                    total_borrowed: 100,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            ),]
        );

        assert_eq!(
            EqBalances::get_balance(&TRADER_EXIST, &asset::BTC),
            SignedBalance::Positive(0),
        );
        assert_eq!(
            EqBalances::get_balance(&borrower_subacc, &asset::BTC),
            SignedBalance::Positive(0),
        );
        assert_eq!(
            EqBalances::get_balance(&trading_acc, &asset::BTC),
            SignedBalance::Positive(0),
        );
        assert_eq!(
            EqBalances::get_balance(&trading_subacc, &asset::BTC),
            SignedBalance::Positive(100),
        );
    });
}

#[test]
fn borrow_overweight() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::set_allocations(
            RuntimeOrigin::root(),
            MM_ID[0],
            vec![(asset::BTC, Perbill::from_percent(75))]
        ));
        assert_ok!(MmPool::add_manager(
            RuntimeOrigin::root(),
            TRADER_EXIST,
            MM_ID[0]
        ));

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            100,
            asset::BTC
        ));
        assert_err!(
            MmPool::borrow(RuntimeOrigin::signed(TRADER_EXIST), 51, asset::BTC),
            Error::Overweight,
        );
        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            50,
            asset::BTC
        ),);
        assert_err!(
            MmPool::borrow(RuntimeOrigin::signed(TRADER_EXIST), 50, asset::BTC),
            Error::Overweight,
        );
    });
}

#[test]
fn borrow_with_pending_withdrawals() {
    new_test_ext().execute_with(|| {
        TimeMock::set_secs(0);
        // Epoch 0

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::ETH, 100));
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::DOT, 100));

        assert_ok!(MmPool::set_allocations(
            RuntimeOrigin::root(),
            MM_ID[0],
            vec![
                (asset::BTC, Perbill::from_percent(100)),
                (asset::ETH, Perbill::from_percent(100)),
                (asset::DOT, Perbill::from_percent(100))
            ]
        ));
        assert_ok!(MmPool::add_manager(
            RuntimeOrigin::root(),
            TRADER_EXIST,
            MM_ID[0]
        ));

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 1000, asset::BTC));
        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 1000, asset::ETH));
        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 1000, asset::DOT));

        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            500,
            asset::BTC
        ));
        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            500,
            asset::ETH
        ));
        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            500,
            asset::DOT
        ));

        assert_ok!(MmPool::request_withdrawal(
            RuntimeOrigin::signed(1),
            500,
            asset::BTC
        ));
        assert_ok!(MmPool::request_withdrawal(
            RuntimeOrigin::signed(1),
            300,
            asset::DOT
        ));

        TimeMock::move_secs(100);
        // Epoch 1
        assert_eq!(
            EqBalances::get_balance(&1, &asset::BTC),
            SignedBalance::Positive(0)
        );
        assert_eq!(
            EqBalances::get_balance(&1, &asset::ETH),
            SignedBalance::Positive(0)
        );
        assert_eq!(
            EqBalances::get_balance(&1, &asset::DOT),
            SignedBalance::Positive(0)
        );

        assert_err!(
            MmPool::borrow(RuntimeOrigin::signed(TRADER_EXIST), 200, asset::BTC),
            Error::NotEnoughToBorrow
        );
        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            200,
            asset::ETH
        ));
        // assert_err!(
        //     MmPool::borrow(RuntimeOrigin::signed(BORROWER_EXIST), 200, asset::DOT),
        //     Error::NotEnoughToBorrow
        // );

        TimeMock::move_secs(100);
        // Epoch 2
        assert_err!(
            MmPool::borrow(RuntimeOrigin::signed(TRADER_EXIST), 200, asset::BTC),
            Error::NotEnoughToBorrow
        );
        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            200,
            asset::DOT
        ));
        assert_ok!(MmPool::withdraw(RuntimeOrigin::signed(1), asset::BTC));
        assert_eq!(
            EqBalances::get_balance(&1, &asset::BTC),
            SignedBalance::Positive(500)
        );

        TimeMock::move_secs(100);
        // Epoch 3
        assert_err!(
            MmPool::borrow(RuntimeOrigin::signed(TRADER_EXIST), 200, asset::BTC),
            Error::NotEnoughToBorrow
        );
        assert_err!(
            MmPool::borrow(RuntimeOrigin::signed(TRADER_EXIST), 200, asset::DOT),
            Error::NotEnoughToBorrow
        );
    });
}

#[test]
fn borrow_and_repay() {
    new_test_ext().execute_with(|| {
        let btc_account_id = MmPool::generate_pool_acc(asset::BTC).unwrap();
        let trading_acc = MmPool::generate_trade_acc(MM_ID[0], &TRADER_EXIST).unwrap();
        let trading_subacc =
            SubaccountsManagerMock::get_subaccount_id(&trading_acc, &SubAccType::Trader).unwrap();

        assert_eq!(
            EqBalances::get_balance(&trading_subacc, &asset::BTC),
            SignedBalance::Positive(0),
        );

        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::set_allocations(
            RuntimeOrigin::root(),
            MM_ID[0],
            vec![(asset::BTC, Perbill::from_percent(100))]
        ));
        assert_ok!(MmPool::add_manager(
            RuntimeOrigin::root(),
            TRADER_EXIST,
            MM_ID[0]
        ));

        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 200,
                    total_deposit: 200,
                    total_borrowed: 0,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            ),]
        );

        assert_ok!(MmPool::borrow(
            RuntimeOrigin::signed(TRADER_EXIST),
            150,
            asset::BTC
        ));
        assert_eq!(
            EqBalances::get_balance(&&trading_subacc, &asset::BTC),
            SignedBalance::Positive(150),
        );
        assert_ok!(MmPool::repay(
            RuntimeOrigin::signed(TRADER_EXIST),
            100,
            asset::BTC
        ));
        assert_err!(
            MmPool::repay(RuntimeOrigin::signed(TRADER_EXIST), 1000, asset::BTC),
            Error::NoFundsToRepay
        );
        assert_eq!(
            EqBalances::get_balance(&trading_subacc, &asset::BTC),
            SignedBalance::Positive(50),
        );

        assert_eq!(
            MmPool::pools(),
            vec![(
                asset::BTC,
                crate::MmPoolInfo {
                    account_id: btc_account_id,
                    min_amount: 100,
                    total_staked: 200,
                    total_deposit: 150,
                    total_borrowed: 50,
                    total_pending_withdrawals: crate::PendingWithdrawal {
                        last_epoch: 0,
                        available: 0,
                        available_next_epoch: 0,
                        requested: 0
                    },
                }
            ),]
        );
    });
}

#[test]
fn create_order_ok() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::set_allocations(
            RuntimeOrigin::root(),
            MM_ID[0],
            vec![(asset::BTC, Perbill::from_percent(100))]
        ));
        assert_ok!(MmPool::add_manager(
            RuntimeOrigin::root(),
            TRADER_EXIST,
            MM_ID[0]
        ));
        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        let asset = asset::BTC;
        let price = FixedI64::from(250);
        let side = OrderSide::Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert_ok!(MmPool::create_order(
            RuntimeOrigin::signed(TRADER_EXIST),
            asset,
            OrderType::Limit {
                price,
                expiration_time
            },
            side,
            amount,
        ));
    });
}

#[test]
fn create_order_not_borrower() {
    new_test_ext().execute_with(|| {
        assert_ok!(MmPool::create_pool(RuntimeOrigin::root(), asset::BTC, 100));
        assert_ok!(MmPool::set_allocations(
            RuntimeOrigin::root(),
            MM_ID[0],
            vec![(asset::BTC, Perbill::from_percent(100))]
        ));
        assert_ok!(MmPool::add_manager(
            RuntimeOrigin::root(),
            TRADER_EXIST,
            MM_ID[0]
        ));
        assert_ok!(MmPool::deposit(RuntimeOrigin::signed(1), 200, asset::BTC));

        let asset = asset::BTC;
        let price = FixedI64::from(250);
        let side = OrderSide::Buy;
        let amount = EqFixedU128::from(1);
        let expiration_time = 100u64;

        assert_err!(
            MmPool::create_order(
                RuntimeOrigin::signed(TRADER_NOT_EXIST),
                asset,
                OrderType::Limit {
                    price,
                    expiration_time
                },
                side,
                amount,
            ),
            Error::BorrowerDoesNotExist
        );
    });
}
