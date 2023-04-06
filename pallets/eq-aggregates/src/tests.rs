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

use super::*;
use crate::mock::*;
use core::slice::Iter;
use eq_primitives::{
    asset, balance::EqCurrency, balance_number::EqFixedU128, eqfxu128, SignedBalance,
    TotalAggregates, UserGroup,
};
use frame_support::{
    assert_ok,
    traits::{ExistenceRequirement, OnKilledAccount, WithdrawReasons},
};
use sp_runtime::FixedPointNumber;

#[test]
fn deposit_withdraw_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;

        for &currency in custom_currency_iterator() {
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                currency,
                eqfxu128!(10, 0).into_inner(),
                true,
                None
            ));
        }

        for &currency in custom_currency_iterator() {
            let total = TotalAggregates {
                collateral: eqfxu128!(10, 0).into_inner(),
                debt: 0,
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                total
            );
        }

        for &currency in custom_currency_iterator() {
            ModuleBalances::deposit_into_existing(
                &account_id_1,
                currency,
                eqfxu128!(10, 0).into_inner(),
                None,
            )
            .expect("deposit_into_existing failed");
        }

        for &currency in custom_currency_iterator() {
            let total = TotalAggregates {
                collateral: eqfxu128!(20, 0).into_inner(),
                debt: 0,
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                total
            );
        }

        for &currency in custom_currency_iterator() {
            ModuleBalances::withdraw(
                &account_id_1,
                currency,
                eqfxu128!(30, 0).into_inner(),
                true,
                None,
                WithdrawReasons::empty(),
                ExistenceRequirement::AllowDeath,
            )
            .expect("withdraw failed");
        }

        for &currency in custom_currency_iterator() {
            let total = TotalAggregates {
                collateral: 0,
                debt: eqfxu128!(10, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                total
            );
        }
    });
}

#[test]
fn transfer_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let account_id_2 = 2;

        for currency in custom_currency_iterator() {
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                *currency,
                eqfxu128!(10, 0).into_inner(),
                true,
                None
            ));
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_2,
                *currency,
                eqfxu128!(20, 0).into_inner(),
                true,
                None
            ));
        }

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: eqfxu128!(30, 0).into_inner(),
                debt: eqfxu128!(00, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );
        }

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_2,
            UserGroup::Bailsmen,
            true
        ));

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: eqfxu128!(30, 0).into_inner(),
                debt: eqfxu128!(00, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );

            let bailsmen_total = TotalAggregates {
                collateral: eqfxu128!(20, 0).into_inner(),
                debt: eqfxu128!(00, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                bailsmen_total
            );
        }

        for currency in custom_currency_iterator() {
            ModuleBalances::currency_transfer(
                &account_id_2,
                &account_id_1,
                *currency,
                eqfxu128!(30, 0).into_inner(),
                ExistenceRequirement::AllowDeath,
                eq_primitives::TransferReason::Common,
                false,
            )
            .expect("currency_transfer failed");
        }

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: eqfxu128!(40, 0).into_inner(),
                debt: eqfxu128!(10, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );

            let bailsmen_total = TotalAggregates {
                collateral: eqfxu128!(0, 0).into_inner(),
                debt: eqfxu128!(10, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                bailsmen_total
            );
        }

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_2,
            UserGroup::Bailsmen,
            false
        ));

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: eqfxu128!(40, 0).into_inner(),
                debt: eqfxu128!(10, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );

            let bailsmen_total = TotalAggregates {
                collateral: eqfxu128!(0, 0).into_inner(),
                debt: eqfxu128!(00, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                bailsmen_total
            );
        }
    });
}

#[test]
fn on_killed_account() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let account_id_2 = 2;
        let balance_1: Balance = eqfxu128!(10, 0).into_inner();
        let balance_2: Balance = eqfxu128!(20, 0).into_inner();

        for currency in custom_currency_iterator() {
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                *currency,
                balance_1,
                true,
                None
            ));
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_2,
                *currency,
                balance_2,
                true,
                None
            ));
        }

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: balance_1 + balance_2,
                debt: 0,
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );
        }

        ModuleAggregates::on_killed_account(&account_id_1);

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: balance_2,
                debt: 0,
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );
        }

        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Balances),
            false
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Balances),
            true
        );

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_2,
            UserGroup::Bailsmen,
            true
        ));

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: balance_2,
                debt: 0,
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                balances_total
            );
        }

        ModuleAggregates::on_killed_account(&account_id_2);

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: 0,
                debt: 0,
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                balances_total
            );
        }
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Balances),
            false
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Bailsmen),
            false
        );
    });
}

#[test]
fn on_killed_account_debt() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let account_id_2 = 2;
        let balance_1: Balance = eqfxu128!(10, 0).into_inner();
        let balance_2: Balance = eqfxu128!(20, 0).into_inner();

        for currency in custom_currency_iterator() {
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                *currency,
                balance_1,
                true,
                None
            ));
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_2,
                *currency,
                balance_2,
                true,
                None
            ));
        }

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: balance_1 + balance_2,
                debt: 0,
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );
        }

        for currency in custom_currency_iterator() {
            ModuleBalances::withdraw(
                &account_id_2,
                *currency,
                eqfxu128!(30, 0).into_inner(),
                true,
                None,
                WithdrawReasons::empty(),
                ExistenceRequirement::AllowDeath,
            )
            .expect("withdraw failed");
        }

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: balance_1,
                debt: eqfxu128!(10, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );
        }

        ModuleAggregates::on_killed_account(&account_id_2);

        for currency in custom_currency_iterator() {
            let balances_total = TotalAggregates {
                collateral: balance_1,
                debt: 0,
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                balances_total
            );
        }
    });
}

#[test]
fn in_usergroup_false() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let account_id_2 = 2;

        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            false
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Balances),
            false
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Bailsmen),
            false
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Balances),
            false
        );
    });
}

#[test]
fn in_usergroup_true() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let account_id_2 = 2;

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_1,
            UserGroup::Bailsmen,
            true
        ));
        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_2,
            UserGroup::Balances,
            true
        ));

        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Bailsmen),
            true
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_1, UserGroup::Balances),
            false
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Bailsmen),
            false
        );
        assert_eq!(
            ModuleAggregates::in_usergroup(&account_id_2, UserGroup::Balances),
            true
        );
    });
}

#[test]
fn empty_aggregates() {
    new_test_ext().execute_with(|| {
        for currency in custom_currency_iterator() {
            let empty = TotalAggregates {
                collateral: eqfxu128!(0, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                empty
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                empty
            );
        }
    });
}

#[test]
fn set_usergroup_add_to_group() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let account_id_2 = 2;

        for currency in custom_currency_iterator() {
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                *currency,
                eqfxu128!(100, 0).into_inner(),
                true,
                None
            ));
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_2,
                *currency,
                eqfxu128!(200, 0).into_inner(),
                true,
                None
            ));
        }

        for currency in custom_currency_iterator() {
            let total = TotalAggregates {
                collateral: eqfxu128!(300, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };
            let empty = TotalAggregates {
                collateral: eqfxu128!(0, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                total
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                empty
            );
        }

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_1,
            UserGroup::Bailsmen,
            true
        ));

        for currency in custom_currency_iterator() {
            let total_balances = TotalAggregates {
                collateral: eqfxu128!(300, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };
            let total_bailsmen = TotalAggregates {
                collateral: eqfxu128!(100, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                total_balances
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                total_bailsmen
            );
        }

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_2,
            UserGroup::Bailsmen,
            true
        ));

        for currency in custom_currency_iterator() {
            let total_balances = TotalAggregates {
                collateral: eqfxu128!(300, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };
            let total_bailsmen = TotalAggregates {
                collateral: eqfxu128!(300, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };

            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                total_balances
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                total_bailsmen
            );
        }
    });
}

#[test]
fn set_usergroup_remove_from_group() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let account_id_2 = 2;
        let empty = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };

        for currency in custom_currency_iterator() {
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                empty
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                empty
            );
        }

        for currency in custom_currency_iterator() {
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                *currency,
                eqfxu128!(100, 0).into_inner(),
                true,
                None
            ));
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_2,
                *currency,
                eqfxu128!(200, 0).into_inner(),
                true,
                None
            ));
        }

        for currency in custom_currency_iterator() {
            let total_balances = TotalAggregates {
                collateral: eqfxu128!(300, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };

            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                total_balances
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                empty
            );
        }

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_1,
            UserGroup::Balances,
            false
        ));

        for currency in custom_currency_iterator() {
            let total_balances = TotalAggregates {
                collateral: eqfxu128!(200, 0).into_inner(),
                debt: eqfxu128!(0, 0).into_inner(),
            };

            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                total_balances
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                empty
            );
        }

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_2,
            UserGroup::Balances,
            false
        ));

        for currency in custom_currency_iterator() {
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                empty
            );
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Bailsmen, *currency),
                empty
            );
        }
    });
}

#[test]
fn update_group_total_zero_balance() {
    new_test_ext().execute_with(|| {
        let signed_balance = SignedBalance::Positive(eqfxu128!(0, 0).into_inner());
        let empty = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };

        for currency in custom_currency_iterator() {
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                empty
            );
        }

        for &currency in custom_currency_iterator() {
            assert_ok!(ModuleAggregates::update_group_total(
                currency,
                &signed_balance,
                &signed_balance,
                UserGroup::Balances,
            ));
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                empty
            );
        }
    });
}

#[test]
fn update_group_total_positive_delta_positive_prev() {
    new_test_ext().execute_with(|| {
        let value = 100;
        let signed_balance = SignedBalance::Positive(eqfxu128!(value, 0).into_inner());
        let aggregate = TotalAggregates {
            collateral: eqfxu128!(value, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };
        let empty = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };

        for currency in custom_currency_iterator() {
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                empty
            );
        }

        for &currency in custom_currency_iterator() {
            assert_ok!(ModuleAggregates::update_group_total(
                currency,
                &signed_balance,
                &signed_balance,
                UserGroup::Balances,
            ));
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                aggregate
            );
        }
    });
}

#[test]
fn update_group_total_positive_delta_negative_prev() {
    new_test_ext().execute_with(|| {
        let debt_value = 300;
        let debt_signed_balance = SignedBalance::Positive(eqfxu128!(debt_value, 0).into_inner());
        let value = 100;
        let signed_balance = SignedBalance::Positive(eqfxu128!(value, 0).into_inner());
        let empty = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };
        let debt_aggregate = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(debt_value, 0).into_inner(),
        };
        let aggregate = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(debt_value - value, 0).into_inner(),
        };

        for currency in custom_currency_iterator() {
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                empty
            );
        }

        // Generate debt at total_user_groups
        for &currency in custom_currency_iterator() {
            assert_ok!(ModuleAggregates::update_group_total(
                currency,
                &debt_signed_balance.negate(),
                &debt_signed_balance.negate(),
                UserGroup::Balances,
            ));
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                debt_aggregate
            );
        }

        for &currency in custom_currency_iterator() {
            assert_ok!(ModuleAggregates::update_group_total(
                currency,
                &signed_balance.negate(),
                &signed_balance,
                UserGroup::Balances,
            ));
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                aggregate
            );
        }
    });
}

#[test]
fn update_group_total_negative_delta_positive_prev() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;

        let prev_value = 100;
        let delta_value = 200;
        let full_value = 500;

        let prev_signed_balance = SignedBalance::Positive(eqfxu128!(prev_value, 0).into_inner());
        let delta_signed_balance = SignedBalance::Positive(eqfxu128!(delta_value, 0).into_inner());

        let empty = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };
        let full = TotalAggregates {
            collateral: eqfxu128!(full_value, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };
        let aggregate = TotalAggregates {
            collateral: eqfxu128!(full_value - prev_value, 0).into_inner(),
            debt: eqfxu128!(delta_value - prev_value, 0).into_inner(),
        };

        for currency in custom_currency_iterator() {
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                empty
            );
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                *currency,
                eqfxu128!(full_value, 0).into_inner(),
                true,
                None
            ));
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                full
            );
        }

        for &currency in custom_currency_iterator() {
            assert_ok!(ModuleAggregates::update_group_total(
                currency,
                &prev_signed_balance,
                &delta_signed_balance.negate(),
                UserGroup::Balances,
            ));
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                aggregate
            );
        }
    });
}

#[test]
fn update_group_total_negative_delta_negative_prev() {
    new_test_ext().execute_with(|| {
        let value = 100;
        let signed_balance = SignedBalance::Positive(eqfxu128!(value, 0).into_inner());
        let aggregate = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(value, 0).into_inner(),
        };

        let empty = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };

        for currency in custom_currency_iterator() {
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, *currency),
                empty
            );
        }

        for &currency in custom_currency_iterator() {
            assert_ok!(ModuleAggregates::update_group_total(
                currency,
                &signed_balance.negate(),
                &signed_balance.negate(),
                UserGroup::Balances,
            ));
            assert_eq!(
                ModuleAggregates::total_user_groups(UserGroup::Balances, currency),
                aggregate
            );
        }
    });
}

#[test]
fn iter_account_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let account_id_2 = 2;

        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_1,
            UserGroup::Bailsmen,
            true
        ));
        assert_ok!(ModuleAggregates::set_usergroup(
            &account_id_2,
            UserGroup::Bailsmen,
            true
        ));

        let ids: Vec<u64> = vec![1, 2];
        let expected = ids.into_iter();
        let actual = ModuleAggregates::iter_account(UserGroup::Bailsmen);
        assert_eq!(actual.eq(expected), true);

        let ids: Vec<u64> = vec![];
        let expected = ids.into_iter();
        let actual = ModuleAggregates::iter_account(UserGroup::Balances);
        assert_eq!(actual.eq(expected), true);
    });
}

#[test]
fn iter_account_empty() {
    new_test_ext().execute_with(|| {
        let ids: Vec<AccountId> = vec![];
        let expected = ids.into_iter();
        let mut actual = ModuleAggregates::iter_account(UserGroup::Bailsmen);
        assert_eq!(actual.eq(expected.clone()), true);
        actual = ModuleAggregates::iter_account(UserGroup::Balances);
        assert_eq!(actual.eq(expected), true);
    });
}

#[test]
fn iter_total_empty() {
    new_test_ext().execute_with(|| {
        let actual = ModuleAggregates::iter_total(UserGroup::Bailsmen);
        let values: Vec<(asset::Asset, TotalAggregates<Balance>)> = vec![];
        let expected = values.into_iter();
        assert_eq!(actual.eq(expected), true);
    });
}

#[test]
fn iter_total_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let value = 200;
        let aggregate = TotalAggregates {
            collateral: eqfxu128!(value, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };

        for currency in custom_currency_iterator() {
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                *currency,
                eqfxu128!(value, 0).into_inner(),
                true,
                None
            ));
        }

        let actual = ModuleAggregates::iter_total(UserGroup::Balances);
        // actual.for_all(|x| println!("{}", x));
        let expected = custom_currency_iterator().map(|c| (*c, aggregate.clone()));

        assert_eq!(actual.eq(expected), true);
    });
}

#[test]
fn get_total_empty() {
    new_test_ext().execute_with(|| {
        let aggregate = TotalAggregates {
            collateral: eqfxu128!(0, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };
        for &currency in custom_currency_iterator() {
            let actual = ModuleAggregates::get_total(UserGroup::Bailsmen, currency);
            assert_eq!(actual, aggregate);
            let actual = ModuleAggregates::get_total(UserGroup::Balances, currency);
            assert_eq!(actual, aggregate);
        }
    });
}

#[test]
fn get_total_success() {
    new_test_ext().execute_with(|| {
        let account_id_1 = 1;
        let value = 300;
        let aggregate = TotalAggregates {
            collateral: eqfxu128!(value, 0).into_inner(),
            debt: eqfxu128!(0, 0).into_inner(),
        };

        for &currency in custom_currency_iterator() {
            assert_ok!(ModuleBalances::deposit_creating(
                &account_id_1,
                currency,
                eqfxu128!(value, 0).into_inner(),
                true,
                None
            ));
        }
        for &currency in custom_currency_iterator() {
            let actual = ModuleAggregates::get_total(UserGroup::Balances, currency);
            assert_eq!(actual, aggregate);
        }
    });
}

fn custom_currency_iterator() -> Iter<'static, asset::Asset> {
    static CURRENCIES: [asset::Asset; 7] = [
        asset::BTC,
        asset::EQ,
        asset::EOS,
        asset::CRV,
        asset::ETH,
        asset::EQD,
        asset::DOT, // Currency::Hbtc,
    ];
    CURRENCIES.iter()
}
