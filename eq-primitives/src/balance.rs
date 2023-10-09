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

use crate::asset::Asset;
use crate::balance_adapter::NegativeImbalance;
use crate::vec_map::VecMap;
use crate::{AccountType, PriceGetter, SignedBalance, TransferReason};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::{BalanceStatus, ExistenceRequirement, LockIdentifier, WithdrawReasons};
use impl_trait_for_tuples::impl_for_tuples;
use sp_arithmetic::traits::AtLeast32BitUnsigned;
use sp_core::sp_std::fmt::Debug;
use sp_runtime::traits::{Get, Member};
use sp_runtime::RuntimeDebug;
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

pub type Balance = u128;

/// An extension to Substrate standard Currency, adapting it to work in
/// Equilibrium substrate
pub trait EqCurrency<AccountId, Balance> {
    /// The quantity used to denote time; usually just a `BlockNumber`.
    type Moment;

    /// The maximum number of locks a user should have on their account.
    type MaxLocks: Get<u32>;
    /// Returns balance value for positive balance and zero for negative
    fn total_balance(who: &AccountId, asset: Asset) -> Balance;

    /// Returns balance value for negative balance and zero for positive
    fn debt(who: &AccountId, asset: Asset) -> Balance;

    /// Gets total issuance of given asset
    fn currency_total_issuance(asset: Asset) -> Balance;

    /// Returns [`ExistentialDeposit`](./trait.Trait.html#associatedtype.ExistentialDeposit)
    fn minimum_balance_value() -> Balance;

    /// Same as `total_balance`
    fn free_balance(who: &AccountId, asset: Asset) -> Balance;

    /// Used to ensure that user's balance in specified asset can be
    /// decreased for `amount`
    fn ensure_can_withdraw(
        who: &AccountId,
        asset: Asset,
        amount: Balance,
        withdraw_reasons: WithdrawReasons,
        new_balance: Balance,
    ) -> DispatchResult;

    /// Operates transfers inside pallet functions
    /// - `transactor` - account sending asset
    /// - `dest` - account receiving asset
    /// - `value` - amount transferred
    /// - `existence_requirement` - currently unused
    /// - `ensure_can_change` - flag for ensuring transfer can be performed with [`can_change_balance`](./trait.BalanceChecker.html#tymethod.can_change_balance)
    fn currency_transfer(
        transactor: &AccountId,
        dest: &AccountId,
        asset: Asset,
        value: Balance,
        existence_requirement: ExistenceRequirement,
        transfer_reason: TransferReason,
        ensure_can_change: bool,
    ) -> DispatchResult;

    /// Adds given amount of asset to account's balance
    fn deposit_into_existing(
        who: &AccountId,
        asset: Asset,
        value: Balance,
        event: Option<DepositReason>,
    ) -> Result<(), DispatchError>;

    /// Performs a deposit creating balance storage for account if it does
    /// not exist
    fn deposit_creating(
        who: &AccountId,
        asset: Asset,
        value: Balance,
        ensure_can_change: bool,
        event: Option<DepositReason>,
    ) -> Result<(), DispatchError>;

    /// Decreases `who` account balance for specified amount of asset
    /// - `ensure_can_change` - flag for ensuring transfer can be performed with
    ///   [`can_change_balance`](./trait.BalanceChecker.html#tymethod.can_change_balance)
    fn withdraw(
        who: &AccountId,
        asset: Asset,
        value: Balance,
        ensure_can_change: bool,
        event: Option<WithdrawReason>,
        withdraw_reasons: WithdrawReasons,
        liveness: ExistenceRequirement,
    ) -> Result<(), DispatchError>;

    /// Force the new free balance of a target account to some new value for tests only
    #[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
    fn make_free_balance_be(who: &AccountId, asset: Asset, value: SignedBalance<Balance>);

    /// Checks whether an account can be deleted
    fn can_be_deleted(who: &AccountId) -> Result<bool, DispatchError>;

    /// Delete account
    fn delete_account(account_id: &AccountId) -> Result<(), DispatchError>;

    /// Exchange assets between two accounts
    /// - `accounts` - accounts that make exchange
    /// - `assets` - assets to exchange
    /// - `values` - amounts of assets \
    /// Error contains `DispatchError` and `Option` with account that failed exchange
    /// because of BalanceChecker checks. Second value in error can be `None`
    /// if error happens for another reason.
    fn exchange(
        accounts: (&AccountId, &AccountId),
        assets: (&Asset, &Asset),
        values: (Balance, Balance),
    ) -> Result<(), (DispatchError, Option<AccountId>)>;

    fn reserved_balance(who: &AccountId, asset: Asset) -> Balance;

    /// Reserve `amount` of `asset` from `who` to special account
    fn reserve(who: &AccountId, asset: Asset, amount: Balance) -> DispatchResult;

    fn slash_reserved(
        who: &AccountId,
        asset: Asset,
        value: Balance,
    ) -> (NegativeImbalance<Balance>, Balance);

    fn repatriate_reserved(
        slashed: &AccountId,
        beneficiary: &AccountId,
        asset: Asset,
        value: Balance,
        status: BalanceStatus,
    ) -> Result<Balance, DispatchError>;

    /// Return reserved balance back to account
    fn unreserve(who: &AccountId, asset: Asset, amount: Balance) -> Balance;

    /// Send asset via xcm
    /// - `kind` - specify does it sends to native location or some another
    /// - `amount` - amount to transfer,
    fn xcm_transfer(
        from: &AccountId,
        asset: Asset,
        amount: Balance,
        kind: XcmDestination,
    ) -> DispatchResult;

    /// Create a new balance lock on account `who`.
    fn set_lock(id: LockIdentifier, who: &AccountId, amount: Balance);

    /// Changes a balance lock (selected by `id`) so that it becomes less liquid in all
    /// parameters or creates a new one if it does not exist.
    fn extend_lock(id: LockIdentifier, who: &AccountId, amount: Balance);

    /// Remove an existing lock.
    fn remove_lock(id: LockIdentifier, who: &AccountId);
}

/// Interface for balance checks
pub trait BalanceChecker<Balance, AccountId, BalanceGetter, SubaccountsManager>
where
    Balance: Debug + Member + Into<u128> + AtLeast32BitUnsigned,
    AccountId: Debug + core::cmp::PartialEq + Decode + Encode,
    BalanceGetter: crate::balance::BalanceGetter<AccountId, Balance>,
    SubaccountsManager: crate::subaccount::SubaccountsManager<AccountId>,
{
    /// Override it for additional checks inside `need_to_check`.
    fn need_to_check_impl(
        _who: &AccountId,
        _changes: &Vec<(Asset, SignedBalance<Balance>)>,
    ) -> bool {
        false
    }
    /// If need to call `can_change_balance_impl` inside `can_change_balance`.
    /// Method makes general checks and not for direct implementation.
    fn need_to_check(who: &AccountId, changes: &Vec<(Asset, SignedBalance<Balance>)>) -> bool {
        if Self::need_to_check_impl(who, changes) {
            return true;
        } else if changes.iter().any(|(_, change)| change.is_negative()) {
            if SubaccountsManager::is_master(who) {
                for (asset, change) in changes.iter() {
                    let new_balance = BalanceGetter::get_balance(who, &asset) + change.clone();
                    match &new_balance {
                        SignedBalance::Positive(_) => (),
                        SignedBalance::Negative(_) => {
                            return true;
                        } // should fail as early as possible
                    }
                }
            } else {
                return true;
            }
        }

        false
    }

    /// Checks whether a specific operation can be performed on user's balance.
    /// Must be implemented.
    fn can_change_balance_impl(
        who: &AccountId,
        changes: &Vec<(Asset, SignedBalance<Balance>)>,
        withdraw_reasons: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError>;

    /// Checks whether a specific operation can be performed on user's balance.
    /// Not for direct implementation.
    fn can_change_balance(
        who: &AccountId,
        changes: &Vec<(Asset, SignedBalance<Balance>)>,
        withdraw_reasons: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        if Self::need_to_check(who, changes) {
            Self::can_change_balance_impl(who, changes, withdraw_reasons)
        } else {
            Ok(())
        }
    }
}

// Tuple implementation for future combination of several BalanceCheckers (you can change balance if all balance checks are passed).
#[impl_for_tuples(5)]
impl<
        Balance: Member + Debug + AtLeast32BitUnsigned,
        AccountId,
        BalanceGetter,
        SubaccountsManager,
    > BalanceChecker<Balance, AccountId, BalanceGetter, SubaccountsManager> for Tuple
where
    Balance: Debug + Member + Into<u128> + AtLeast32BitUnsigned,
    AccountId: Debug + core::cmp::PartialEq + Decode + Encode,
    BalanceGetter: crate::balance::BalanceGetter<AccountId, Balance>,
    SubaccountsManager: crate::subaccount::SubaccountsManager<AccountId>,
{
    fn can_change_balance(
        who: &AccountId,
        changes: &Vec<(Asset, SignedBalance<Balance>)>,
        reason: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        for_tuples!( #( Tuple::can_change_balance(who, changes, reason)?; )* );

        Ok(())
    }

    fn can_change_balance_impl(
        who: &AccountId,
        changes: &Vec<(Asset, SignedBalance<Balance>)>,
        reason: Option<WithdrawReasons>,
    ) -> Result<(), sp_runtime::DispatchError> {
        for_tuples!( #( Tuple::can_change_balance_impl(who, changes, reason)?; )* );

        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct DebtCollateralDiscounted<Balance> {
    pub debt: Balance,
    pub collateral: Balance,
    pub discounted_collateral: Balance,
}

/// Balances reading interface
pub trait BalanceGetter<AccountId, Balance>
where
    Balance: Debug + Member + Into<crate::balance::Balance> + AtLeast32BitUnsigned,
    AccountId: Debug,
{
    type Iterator: Iterator<Item = (Asset, SignedBalance<Balance>)>;
    type PriceGetter: PriceGetter;
    /// Gets account `who` balance for given asset
    fn get_balance(who: &AccountId, asset: &Asset) -> SignedBalance<Balance>;

    /// Used for iteration over whole balances storage. DO NOT USE IN RUNTIME,
    /// only for offchain workers
    fn iterate_balances() -> BTreeMap<AccountId, Vec<(Asset, SignedBalance<Balance>)>>;

    /// Used to iterate over each asset balance of `account`
    fn iterate_account_balances(account: &AccountId) -> VecMap<Asset, SignedBalance<Balance>>;

    /// Gets total value of account's debt and collateral in USD
    /// Returns:
    /// - debt and collateral for all assets
    /// - discounted collateral
    fn get_debt_and_collateral(
        who: &AccountId,
    ) -> Result<DebtCollateralDiscounted<Balance>, DispatchError>;
}

pub trait BalanceRemover<AccountId>
where
    AccountId: Debug,
{
    /// Removes empty balance for asset
    fn remove_asset(who: AccountId, asset_to_remove: &Asset) -> Result<(), DispatchError>;
}

/// Function for calculations: splits a `value` into 2 parts: `amount` and
/// `value` minus `amount`. If `amount` > `value`, it returns value and zero
fn split<Balance>(value: Balance, amount: Balance) -> (Balance, Balance)
where
    Balance: AtLeast32BitUnsigned + Copy,
{
    let first = value.min(amount);
    let second = value - first;
    (first, second)
}

/// Function for calculations: splits `value` into 2 parts in given
/// proportions of `first` and `second`
pub fn ration<Balance>(value: Balance, first: u32, second: u32) -> (Balance, Balance)
where
    Balance: AtLeast32BitUnsigned + Copy,
{
    let total = first.saturating_add(second);
    let amount1 = value.saturating_mul(first.into()) / total.into();
    split(value, amount1)
}

#[cfg(test)]
mod tests {
    use core::convert::{TryFrom, TryInto};

    use frame_support::{assert_err, assert_ok, storage::PrefixIterator};
    use sp_runtime::{
        traits::{One, Zero},
        FixedI64, FixedPointNumber,
    };

    use crate::subaccount::SubaccountsManager;

    use super::*;

    struct OracleMock;
    impl PriceGetter for OracleMock {
        fn get_price<F: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>>(
            _asset: &Asset,
        ) -> Result<F, sp_runtime::DispatchError> {
            FixedI64::one()
                .try_into()
                .map_err(|_| sp_runtime::DispatchError::Other("Unexpected"))
        }
    }

    mod accounts {
        pub const NEED_CUSTOM_CHECK: u64 = 1;
        pub const NO_NEED_CUSTOM_CHECK: u64 = 2;
        pub const MASTER: u64 = 3;
    }

    struct BalanceGetterMock;

    impl BalanceGetter<u64, crate::balance::Balance> for BalanceGetterMock {
        type PriceGetter = OracleMock;
        type Iterator = PrefixIterator<(Asset, SignedBalance<crate::balance::Balance>)>;

        fn get_balance(who: &u64, _asset: &Asset) -> SignedBalance<crate::balance::Balance> {
            match who {
                &accounts::MASTER => SignedBalance::Positive(1),
                &accounts::NO_NEED_CUSTOM_CHECK => SignedBalance::Positive(2),
                _ => SignedBalance::zero(),
            }
        }

        fn iterate_balances() -> BTreeMap<u64, Vec<(Asset, SignedBalance<crate::balance::Balance>)>>
        {
            unimplemented!()
        }

        fn iterate_account_balances(
            _account: &u64,
        ) -> crate::vec_map::VecMap<Asset, SignedBalance<crate::balance::Balance>> {
            unimplemented!()
        }

        fn get_debt_and_collateral(
            _who: &u64,
        ) -> Result<DebtCollateralDiscounted<crate::balance::Balance>, DispatchError> {
            unimplemented!()
        }
    }

    struct SubaccountsManagerMock;
    impl SubaccountsManager<u64> for SubaccountsManagerMock {
        fn create_subaccount_inner(
            _who: &u64,
            _subacc_type: &crate::subaccount::SubAccType,
        ) -> Result<u64, DispatchError> {
            unimplemented!()
        }

        fn delete_subaccount_inner(
            _who: &u64,
            _subacc_type: &crate::subaccount::SubAccType,
        ) -> Result<u64, DispatchError> {
            unimplemented!()
        }

        fn has_subaccount(_who: &u64, _subacc_type: &crate::subaccount::SubAccType) -> bool {
            unimplemented!()
        }

        fn get_subaccount_id(
            _who: &u64,
            _subacc_type: &crate::subaccount::SubAccType,
        ) -> Option<u64> {
            unimplemented!()
        }

        fn is_subaccount(_who: &u64, _subaccount_id: &u64) -> bool {
            unimplemented!()
        }

        fn get_owner_id(_subaccount: &u64) -> Option<(u64, crate::subaccount::SubAccType)> {
            unimplemented!()
        }

        fn get_subaccounts_amount(_who: &u64) -> usize {
            unimplemented!()
        }

        fn is_master(who: &u64) -> bool {
            match who {
                &accounts::MASTER
                | &accounts::NO_NEED_CUSTOM_CHECK
                | &accounts::NEED_CUSTOM_CHECK => true,
                _ => false,
            }
        }
    }

    struct BalanceCheckerCustomNeedToCheck;

    impl BalanceChecker<crate::balance::Balance, u64, BalanceGetterMock, SubaccountsManagerMock>
        for BalanceCheckerCustomNeedToCheck
    {
        fn need_to_check_impl(
            who: &u64,
            _changes: &Vec<(Asset, SignedBalance<crate::balance::Balance>)>,
        ) -> bool {
            who == &accounts::NEED_CUSTOM_CHECK
        }
        fn can_change_balance_impl(
            _who: &u64,
            _changes: &Vec<(Asset, SignedBalance<crate::balance::Balance>)>,
            _reason: Option<WithdrawReasons>,
        ) -> Result<(), sp_runtime::DispatchError> {
            Err(sp_runtime::DispatchError::Other("Custom error"))
        }
    }

    struct BalanceCheckerDefaultNeedToCheck;

    impl BalanceChecker<crate::balance::Balance, u64, BalanceGetterMock, SubaccountsManagerMock>
        for BalanceCheckerDefaultNeedToCheck
    {
        fn can_change_balance_impl(
            _who: &u64,
            _changes: &Vec<(Asset, SignedBalance<crate::balance::Balance>)>,
            _reason: Option<WithdrawReasons>,
        ) -> Result<(), sp_runtime::DispatchError> {
            Ok(())
        }
    }

    #[test]
    fn balance_checker() {
        let negative_changes = &vec![(crate::asset::EQD, SignedBalance::Negative(2))];
        let positive_changes = &vec![(crate::asset::EQD, SignedBalance::Positive(1))];
        assert_ok!(<(
            BalanceCheckerDefaultNeedToCheck,
            BalanceCheckerCustomNeedToCheck
        )>::can_change_balance(
            &accounts::NO_NEED_CUSTOM_CHECK,
            &negative_changes,
            None,
        ));

        assert_err!(
            <(
                BalanceCheckerDefaultNeedToCheck,
                BalanceCheckerCustomNeedToCheck
            )>::can_change_balance(
                &accounts::NEED_CUSTOM_CHECK, &negative_changes, None,
            ),
            sp_runtime::DispatchError::Other("Custom error")
        );

        assert_ok!(BalanceCheckerDefaultNeedToCheck::can_change_balance(
            &accounts::NEED_CUSTOM_CHECK,
            negative_changes,
            None,
        ));

        assert_err!(
            <(
                BalanceCheckerDefaultNeedToCheck,
                BalanceCheckerCustomNeedToCheck
            )>::can_change_balance(&accounts::MASTER, &negative_changes, None),
            sp_runtime::DispatchError::Other("Custom error")
        );

        assert_ok!(<(
            BalanceCheckerDefaultNeedToCheck,
            BalanceCheckerCustomNeedToCheck
        )>::can_change_balance(
            &accounts::MASTER,
            &vec![(crate::asset::EQD, SignedBalance::Negative(1))],
            None,
        ));

        for acc in [accounts::MASTER, accounts::NO_NEED_CUSTOM_CHECK] {
            assert_ok!(<(
                BalanceCheckerDefaultNeedToCheck,
                BalanceCheckerCustomNeedToCheck
            )>::can_change_balance(
                &acc, &positive_changes, None
            ));
        }
    }

    #[test]
    fn split_works() {
        let a = 100u64;
        let b = 20u64;

        let (first_part, second_part) = split(a, b);
        assert_eq!(first_part + second_part, a);
        assert_eq!(first_part, 20u64);
        assert_eq!(second_part, 80u64);
    }

    #[test]
    fn ration_works() {
        let (first, second) = ration(100, 15u32, 85u32);
        assert_eq!(first + second, 100);
        assert_eq!(first, 15u64);
        assert_eq!(second, 85u64);

        let (first, second) = ration(100, 15u32, 45u32);
        assert_eq!(first + second, 100);
        assert_eq!(first, 25u64);
        assert_eq!(second, 75u64);

        let (first, second) = ration(100, 1u32, 2u32);
        assert_eq!(first + second, 100);
        assert_eq!(first, 33u64);
        assert_eq!(second, 67u64);
    }
}

pub enum XcmDestination {
    /// Send to some multilocation
    Common(xcm::v3::MultiLocation),
    /// Send to native asset location (example: send KSM to account on Kusama chain)
    Native(AccountType),
}

#[derive(
    Clone, Copy, PartialEq, Eq, Encode, Decode, RuntimeDebug, MaxEncodedLen, scale_info::TypeInfo,
)]
pub enum XcmTransferDealWithFee {
    /// Send fee to Treasury on local chain and force sovereign acc to pay fee on target chain.
    /// Assumes that sovereign has enough tokens to pay fee.
    /// If asset to pay fee differs from asset to transfer than it is converting according to prices.
    SovereignAccWillPay,
    /// Send xcm message and force sender to pay fee from his account on target chain.
    /// Xcm message payload could be changed.
    AccOnTargetChainWillPay,
    /// Send xcm message and force sender to pay fee (in xcm_fee_asset without converting) from his account on local chain.
    /// Withdrawed fee amount should be send in the xcm to pay fee on target chain.
    ThisAccWillPay,
}

#[derive(Debug, Clone, Copy, PartialEq, Decode, Encode, scale_info::TypeInfo)]
pub enum DepositReason {
    /// External call for mint
    Extrinsic,
    /// Create or deposit from reverved account while receiving XCM
    XcmTransfer,
    /// XCM fee while receiving XCM
    XcmPayment,
    /// Mint while removing asset
    AssetRemoval,
    /// Mint while staking
    Staking,
    /// Swap XDOT to DOT
    XDotSwap,
}

#[derive(Debug, Clone, Copy, PartialEq, Decode, Encode, scale_info::TypeInfo)]
pub enum WithdrawReason {
    /// External call for burn
    Extrinsic,
    /// Withdraw fund from reserved account while receiving XCM
    XcmReserve,
    /// Burn assets while sending XCM
    XcmTransfer,
    /// XCM fee payment
    XcmPayment,
    /// Burn while removing asset
    AssetRemoval,
    /// Burn while staking DOT
    Staking,
    /// Swap XDOT to DOT
    XDotSwap,
}

#[derive(
    Debug, Clone, Eq, PartialEq, Decode, Encode, scale_info::TypeInfo, codec::MaxEncodedLen,
)]
pub enum AccountData<Balance> {
    V0 {
        lock: Balance,
        balance: VecMap<Asset, SignedBalance<Balance>>,
    },
}

impl<Balance: Default> Default for AccountData<Balance> {
    fn default() -> Self {
        AccountData::V0 {
            lock: Default::default(),
            balance: Default::default(),
        }
    }
}

impl<Balance> AccountData<Balance> {
    pub fn get(&self, asset: &Asset) -> SignedBalance<Balance>
    where
        Balance: Clone + Default,
    {
        match self {
            Self::V0 { balance, lock: _ } => balance.get(asset).cloned().unwrap_or_default(),
        }
    }

    pub fn entry(
        &mut self,
        asset: Asset,
    ) -> crate::vec_map::entry::Entry<'_, Asset, SignedBalance<Balance>> {
        match self {
            Self::V0 { balance, lock: _ } => balance.entry(asset),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Asset, &SignedBalance<Balance>)> {
        match self {
            Self::V0 { balance, lock: _ } => balance.iter(),
        }
    }
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&Asset, &mut SignedBalance<Balance>) -> bool,
    {
        match self {
            Self::V0 { balance, lock: _ } => balance.retain(f),
        }
    }
}

pub trait LockGetter<AccountId, Balance> {
    fn get_lock(who: AccountId, id: LockIdentifier) -> Balance;
}
