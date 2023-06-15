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

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(warnings)]

use balance::Balance;
use balance_number::EqFixedU128;
use frame_support::{
    codec::{Decode, Encode, FullCodec},
    dispatch::{DispatchError, DispatchResult, DispatchResultWithPostInfo},
};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize, Member},
    transaction_validity::TransactionPriority,
    FixedI64, RuntimeDebug,
};
use sp_std::convert::TryInto;
use sp_std::fmt::Debug;
use sp_std::prelude::*;

use asset::Asset;
pub use polkadot_core_primitives::Balance as XcmBalance;
use xcm::v3::Junction::*;

pub use crate::bailsman::*;
pub use crate::dex::*;
pub use crate::price::{PriceGetter, PriceSetter};
pub use crate::signed_balance::SignedBalance;

pub mod asset;
pub mod bailsman;
pub mod bailsman_redistribute_weight;
pub mod balance;
pub mod balance_adapter;
pub mod balance_number;
pub mod chainbridge;
pub mod curve_number;
pub mod dex;
pub mod financial_storage;
pub mod imbalances;
#[cfg(feature = "std")]
pub mod mocks;
pub mod offchain_batcher;
pub mod price;
pub mod proxy;
pub mod signed_balance;
pub mod subaccount;
pub mod vec_map;
pub mod wrapped_dot;
pub mod xcm_origins;
pub mod xdot_pool;

pub const ONE_TOKEN: Balance = 1_000_000_000;
pub const DECIMALS: u8 = 9;

/// User groups used in Equilibrium substrate
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    RuntimeDebug,
    Hash,
    PartialOrd,
    Ord,
    scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum UserGroup {
    /// User can have balances and transfer currencies
    Balances = 1,
    /// User is a bailsman
    Bailsmen = 2,
    /// User is a borrower
    Borrowers = 3,
}

impl Eq for UserGroup {}

impl UserGroup {
    /// Iterator over user groups
    pub fn iterator() -> impl Iterator<Item = UserGroup> {
        IntoIterator::into_iter([
            UserGroup::Balances,
            UserGroup::Bailsmen,
            UserGroup::Borrowers,
        ])
    }
}

/// Aggregated values of positive (collateral) and negative (debt) balances
/// for a group of users
#[derive(Encode, Decode, Clone, Default, PartialEq, Debug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TotalAggregates<Balance> {
    pub collateral: Balance,
    pub debt: Balance,
}

/// When implemented, pallet can work with `TotalAggregates`
pub trait Aggregates<AccountId, Balance>
where
    Balance: Member
        + AtLeast32BitUnsigned
        + FullCodec
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + Default,
{
    /// Checks wether `account_id` is in a group `user_group`
    fn in_usergroup(account_id: &AccountId, user_group: UserGroup) -> bool;

    /// Adds `account_id` to a `user_group` or removes it from a group based on
    /// passed `is_in` bool
    fn set_usergroup(account_id: &AccountId, user_group: UserGroup, is_in: bool) -> DispatchResult;

    /// Updates `TotalAggregates` of a group when balance of `account_id`
    /// changes for `delta_balance`
    fn update_total(
        account_id: &AccountId,
        asset: Asset,
        prev_balance: &SignedBalance<Balance>,
        delta_balance: &SignedBalance<Balance>,
    ) -> DispatchResult;

    /// Used to iterate over all accounts of a user group
    fn iter_account(user_group: UserGroup) -> Box<dyn Iterator<Item = AccountId>>;

    /// Used to iterate over currency total of a user group
    fn iter_total(
        user_group: UserGroup,
    ) -> Box<dyn Iterator<Item = (Asset, TotalAggregates<Balance>)>>;

    /// Used to get total by user group and currency
    fn get_total(user_group: UserGroup, asset: Asset) -> TotalAggregates<Balance>;
}

pub trait AggregatesAssetRemover {
    fn remove_asset(asset: &Asset);
}

impl AggregatesAssetRemover for () {
    fn remove_asset(_: &Asset) {}
}

#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    RuntimeDebug,
    Hash,
    PartialOrd,
    Ord,
    scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[repr(u8)]
/// Describe the reason for the transfer.
pub enum TransferReason {
    /// Usual transfer from user to another.
    Common,

    /// Charging Interest fee for borrowing assets.
    InterestFee,

    /// Margincall of user position.
    MarginCall,

    /// Liquidity Farming's transfer to the bailsmen temp balances.
    LiquidityFarming,

    /// Redistribution of bailsmen temp balances.
    BailsmenRedistribution,

    /// Buyout treasury eq tokens to pay off borrower interest (if user has not enough eq tokens on balance).
    TreasuryEqBuyout,

    /// Transfer between a main account and one of it's subaccounts
    Subaccount,

    /// Transfer to Lockdrop pallet for staking
    Lock,

    /// Transfer from Lockdrop pallet for unstaking
    Unlock,

    /// Transfer from Crowdloan pallet to add gens to account
    Claim,

    /// Transfer from CurveAmm pallet to CurveDistribution pallet
    CurveFeeWithdraw,

    /// Balance reservation
    Reserve,

    /// Return balance from reserve
    Unreserve,

    /// XCM transfer
    XcmTransfer,

    /// XCM fee payment
    XcmPayment,
}

impl Eq for TransferReason {}

impl Default for TransferReason {
    fn default() -> TransferReason {
        TransferReason::Common
    }
}

pub trait IsTransfersEnabled {
    fn get() -> bool;
}

/// Manager for treasury Eq exchanging transactions
pub trait EqBuyout<AccountId, Balance> {
    /// Buyout `amount` of Eq from Treasury. Account `who` pays for it with it's
    /// balances accordingly to exchange priority (see `get_currency_priority`)
    fn eq_buyout(who: &AccountId, amount: Balance) -> DispatchResult;

    /// Check if `amount` of `currency` is enough to buyout `amount_buyout` of Eq from Treasury
    fn is_enough(
        asset: Asset,
        amount: Balance,
        amount_buyout: Balance,
    ) -> Result<bool, DispatchError>;
}
pub trait LendingPoolManager<Balance> {
    /// Adds new rewards in lending pool
    fn add_reward(asset: Asset, reward: Balance) -> DispatchResult;
}

/// Empty implementation for using in unit tests
impl<Balance> LendingPoolManager<Balance> for () {
    fn add_reward(_: Asset, _: Balance) -> DispatchResult {
        Ok(())
    }
}

pub trait LendingAssetRemoval<AccountId> {
    /// Removes all entires with asset from eq_lending::{LendersAggregates, CumulatedRewards} storages
    fn remove_from_aggregates_and_rewards(asset: &Asset);
    /// Removes all entries with asset from eq_lending::Lenders storage
    fn remove_from_lenders(asset: &Asset, account: &AccountId);
}

/// Empty implementation for using in unit tests
impl<AccountId> LendingAssetRemoval<AccountId> for () {
    fn remove_from_aggregates_and_rewards(_: &Asset) {}
    fn remove_from_lenders(_: &Asset, _: &AccountId) {}
}
/// Equilibrium Rate pallet trait, used to set timestamp of account last update.
/// Used for reinits and fee calculations
pub trait UpdateTimeManager<AccountId> {
    /// Sets current time as last update time for given accounts
    fn set_last_update(account_id: &AccountId);
    /// Sets time as last update time for given accounts
    #[cfg(not(feature = "production"))]
    fn set_last_update_timestamp(account_id: &AccountId, timestamp_ms: u64);
    /// Removes information about last update time. Used for deleting accounts.
    fn remove_last_update(account_id: &AccountId);
}

/// Used for dealing with `providers` and `consumers` Account counters.
pub trait AccountRefCounts<AccountId> {
    /// Increment all counters for account `who`
    fn inc_ref(who: &AccountId);
    /// Decrement all counters for account `who`
    fn dec_ref(who: &AccountId);
    /// Double check subaccounts amount!
    /// Check, if all counters for account `who` are zeroes
    fn can_be_deleted(who: &AccountId, subaccounts_amount: usize) -> bool;
}

pub struct AccountRefCounter<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> AccountRefCounts<T::AccountId> for AccountRefCounter<T> {
    fn inc_ref(who: &T::AccountId) {
        frame_system::Pallet::<T>::inc_providers(who);
        frame_system::Pallet::<T>::inc_consumers_without_limit(who)
            .expect("Unexpected inc_consumers_without_limit error");
    }
    fn dec_ref(who: &T::AccountId) {
        frame_system::Pallet::<T>::dec_consumers(who);
        frame_system::Pallet::<T>::dec_providers(who).expect("Unexpected dec_providers error");
    }
    fn can_be_deleted(who: &T::AccountId, subaccounts_amount: usize) -> bool {
        frame_system::Pallet::<T>::consumers(who) == 0
            && frame_system::Pallet::<T>::providers(who) == subaccounts_amount as u32 + 1
    }
}

/// Used for initialization module accounts
pub trait PalletAccountInitializer<AccountId> {
    ///Initialize module account
    fn initialize(who: &AccountId);
}

pub struct EqPalletAccountInitializer<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> PalletAccountInitializer<T::AccountId>
    for EqPalletAccountInitializer<T>
{
    /// We call inc_providers two times here to differentiate that this account id
    /// belongs to module and not to regular account.
    /// This is done so that the transfer will not call buyout depending on amount of
    /// refcounts
    fn initialize(who: &T::AccountId) {
        frame_system::Pallet::<T>::inc_providers(who);
        frame_system::Pallet::<T>::inc_providers(who);
        frame_system::Pallet::<T>::inc_consumers(who).expect("Unexpected inc_consumers error");
    }
}

//------------- for eq-margin-call --------------------
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum MarginState {
    /// x >= initial_margin
    Good,
    ///  maitenance_margin  <= x <  initial_margin and no maintenance timer started
    SubGood,
    /// Maintenance margin call is to start
    MaintenanceStart,
    /// Maintenance margin call has occured
    MaintenanceIsGoing,
    /// Maintenance timer is over, MC is now imminent
    MaintenanceTimeOver,
    /// Margin has been topped up, maintenance timer is deleted
    MaintenanceEnd,
    /// x < critical_margin, MC is now imminent
    SubCritical,
}

impl MarginState {
    // no need to margincall this position
    pub fn good_position(&self) -> bool {
        matches!(
            self,
            Self::Good | Self::SubGood | Self::MaintenanceIsGoing | Self::MaintenanceEnd
        )
    }
}

pub trait MarginCallManager<AccountId, Balance>
where
    Balance: Member + Debug,
{
    fn check_margin_with_change(
        owner: &AccountId,
        balance_changes: &[BalanceChange<Balance>],
        order_changes: &[OrderChange],
    ) -> Result<(MarginState, bool), DispatchError>;

    /// Diagnoses the current margin state of an account with no side effects
    fn check_margin(owner: &AccountId) -> Result<MarginState, DispatchError> {
        let (margin_state, _) = Self::check_margin_with_change(owner, &[], &[])?;
        Ok(margin_state)
    }

    fn try_margincall(owner: &AccountId) -> Result<MarginState, DispatchError>;

    fn get_critical_margin() -> EqFixedU128;
}

/// Equilibrium Vesting pallet trait used to update accounts locks
pub trait Vesting<AccountId> {
    fn update_vest_lock(who: AccountId) -> DispatchResultWithPostInfo;

    fn has_vesting_schedule(who: AccountId) -> bool;
}

pub trait Crowdloan<AccountId, Balance> {
    /// Move allocation to another destination
    /// Returns tuple of (allocation, transfer_amount, Option<penalty_amount>)
    fn move_crowdloan_allocation(
        who: &AccountId,
        dest: &AccountId,
        penalty: bool,
    ) -> Result<(Balance, Balance, Option<Balance>), DispatchError>;

    /// Returns allocation amount of `who`
    fn allocation_amount(who: &AccountId) -> Balance;

    /// Check if allocation exists for `who`
    fn allocation_exists(who: &AccountId) -> bool;
}

#[derive(Clone, Debug)]
pub struct OrderChange {
    pub asset: Asset,
    pub amount: EqFixedU128,
    pub price: FixedI64,
    pub side: OrderSide,
}

#[derive(Clone, Debug)]
pub struct BalanceChange<Balance: Member> {
    pub change: SignedBalance<Balance>,
    pub asset: Asset,
}

/// UnsignedPriorityPair = (TransactionPriority, MinTransactionWeight)
/// Unsigned priority = TransactionPriority + block_number % MinTransactionWeight
pub type UnsignedPriorityPair = (TransactionPriority, u64);

pub fn calculate_unsigned_priority<BlockNumber>(
    params: &UnsignedPriorityPair,
    block_number: BlockNumber,
) -> TransactionPriority
where
    BlockNumber: AtLeast32BitUnsigned,
{
    let (initial_priority, min_transaction_weight) = params;
    initial_priority.saturating_add(
        (TryInto::<u64>::try_into(block_number).unwrap_or(0)) % *min_transaction_weight,
    )
}

#[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Deserialize, Serialize))]
pub enum AccountType {
    Id32([u8; 32]),
    Key20([u8; 20]),
}

impl sp_std::convert::TryFrom<Vec<u8>> for AccountType {
    type Error = ();

    fn try_from(value: Vec<u8>) -> Result<Self, ()> {
        match value.len() {
            20 => {
                let mut key = [0u8; 20];
                key.copy_from_slice(&value[..]);
                Ok(Self::Key20(key))
            }
            32 => {
                let mut id = [0u8; 32];
                id.copy_from_slice(&value[..]);
                Ok(Self::Id32(id))
            }
            _ => Err(()),
        }
    }
}

impl Into<Vec<u8>> for AccountType {
    fn into(self) -> Vec<u8> {
        match &self {
            AccountType::Id32(id) => id.to_vec(),
            AccountType::Key20(key) => key.to_vec(),
        }
    }
}

impl AccountType {
    pub fn multi_location(self) -> xcm::v3::Junction {
        match self {
            AccountType::Id32(id) => AccountId32 { network: None, id },
            AccountType::Key20(key) => AccountKey20 { network: None, key },
        }
    }
}

pub struct BlockNumberToBalance;

impl sp_runtime::traits::Convert<u32, Balance> for BlockNumberToBalance {
    fn convert(block_number: u32) -> Balance {
        block_number as Balance
    }
}

#[derive(Decode, Encode, Copy, Clone, Debug, PartialEq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
/// Determines the xcm executing mode on parachain:\
/// - `Bridge` will call bridge transfer on receiving, disallow to send xcm for users\
/// - `Xcm` normal executing on receiving, will call EqCurrecny methods, allow to send xcm for users\
/// - Internal bool value means for enable / disable sending
pub enum XcmMode {
    Bridge(bool),
    Xcm(bool),
}

/// Account used to distribute balances to bailsmen.
/// Used by eq_bailsman & eq_rate pallets.
pub const DISTRIBUTION_ACC: frame_support::PalletId = frame_support::PalletId(*b"distbail");
