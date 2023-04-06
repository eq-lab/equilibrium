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

use codec::FullCodec;
use eq_primitives::{
    asset::{self, Asset},
    balance_number::EqFixedU128,
    signed_balance::EqMember,
    xdot_pool::XBasePrice,
    BailsmanManager, MarginCallManager, OrderAggregateBySide, PriceGetter, TransferReason,
};
use eq_utils::vec_map::VecMap;
use financial_primitives::{CalcReturnType, CalcVolatilityType};
use frame_support::{
    pallet_prelude::{MaybeSerializeDeserialize, Member},
    traits::{ExistenceRequirement, Get},
};
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, One, Zero},
    DispatchError, FixedI64, FixedPointNumber,
};
use sp_std::{convert::TryFrom, fmt::Debug, marker::PhantomData, prelude::*};

pub struct PriceGetterMock<AssetGetter>(PhantomData<AssetGetter>);
impl<AssetGetter: asset::AssetGetter> PriceGetter for PriceGetterMock<AssetGetter> {
    fn get_price<FixedNumber>(asset: &Asset) -> Result<FixedNumber, sp_runtime::DispatchError>
    where
        FixedNumber: FixedPointNumber + One + Zero + Debug + TryFrom<FixedI64>,
    {
        AssetGetter::get_asset_data(asset)?;
        Ok(FixedNumber::one())
    }
}

pub struct BalanceAwareMock<AccountId, Asset>(PhantomData<(AccountId, Asset)>);
impl<AccountId, Asset> financial_primitives::BalanceAware for BalanceAwareMock<AccountId, Asset> {
    type AccountId = AccountId;
    type Asset = Asset;
    type Balance = substrate_fixed::FixedI128<substrate_fixed::types::extra::U64>;
    fn balances(
        _account_id: &Self::AccountId,
        _assets: &[Self::Asset],
    ) -> Result<sp_std::vec::Vec<Self::Balance>, DispatchError> {
        Ok(sp_std::vec::Vec::<_>::new())
    }
}

pub struct BailsmanManagerMock<TreasuryAccount, EqCurrency>(
    PhantomData<(TreasuryAccount, EqCurrency)>,
);
impl<
        AccountId,
        Balance,
        TreasuryAccount: Get<AccountId>,
        EqCurrency: eq_primitives::balance::EqCurrency<AccountId, Balance>,
    > BailsmanManager<AccountId, Balance> for BailsmanManagerMock<TreasuryAccount, EqCurrency>
where
    Balance: EqMember
        + AtLeast32BitUnsigned
        + FullCodec
        + Copy
        + MaybeSerializeDeserialize
        + Debug
        + Default,
{
    fn register_bailsman(_who: &AccountId) -> Result<(), sp_runtime::DispatchError> {
        Err(sp_runtime::DispatchError::Other("register bailsman"))
    }

    fn unregister_bailsman(_who: &AccountId) -> Result<(), sp_runtime::DispatchError> {
        Err(sp_runtime::DispatchError::Other("unreg bailsman"))
    }

    fn receive_position(who: &AccountId, _is_deleting_position: bool) -> Result<(), DispatchError> {
        let eq_balance = EqCurrency::total_balance(&who, asset::EQ);
        EqCurrency::currency_transfer(
            who,
            &TreasuryAccount::get(),
            asset::EQ,
            eq_balance,
            ExistenceRequirement::KeepAlive,
            TransferReason::Common,
            true,
        )
    }

    fn redistribute(_who: &AccountId) -> Result<u32, sp_runtime::DispatchError> {
        Ok(0)
    }

    fn get_account_distribution(
        _who: &AccountId,
    ) -> Result<eq_primitives::AccountDistribution<Balance>, sp_runtime::DispatchError> {
        Ok(eq_primitives::AccountDistribution {
            transfers: Default::default(),
            last_distribution_id: Default::default(),
            current_distribution_id: Default::default(),
            new_queue: Default::default(),
        })
    }

    fn should_unreg_bailsman(
        _who: &AccountId,
        _amounts: &[(Asset, eq_primitives::SignedBalance<Balance>)],
        _debt_and_allowed_collateral: Option<(Balance, Balance)>,
    ) -> Result<bool, sp_runtime::DispatchError> {
        Ok(false)
    }

    fn bailsmen_count() -> u32 {
        0
    }

    fn distribution_queue_len() -> u32 {
        0
    }
}

pub struct MarginCallManagerMock;
impl<AccountId, Balance> MarginCallManager<AccountId, Balance> for MarginCallManagerMock
where
    Balance: Member + Debug,
{
    fn check_margin_with_change(
        _owner: &AccountId,
        _balance_changes: &[eq_primitives::BalanceChange<Balance>],
        _order_changes: &[eq_primitives::OrderChange],
    ) -> Result<(eq_primitives::MarginState, bool), DispatchError> {
        Ok((eq_primitives::MarginState::Good, true))
    }

    fn check_margin(_owner: &AccountId) -> Result<eq_primitives::MarginState, DispatchError> {
        Ok(eq_primitives::MarginState::Good)
    }

    fn try_margincall(_owner: &AccountId) -> Result<eq_primitives::MarginState, DispatchError> {
        Ok(eq_primitives::MarginState::Good)
    }

    fn get_critical_margin() -> EqFixedU128 {
        EqFixedU128::saturating_from_rational(5, 1000)
    }
}

pub struct XbasePriceMock<Asset, Balance, PriceNumber>(PhantomData<(Asset, Balance, PriceNumber)>);
impl<Asset: core::default::Default, Balance: core::default::Default, PriceNumber>
    XBasePrice<Asset, Balance, PriceNumber> for XbasePriceMock<Asset, Balance, PriceNumber>
{
    type XdotPoolInfo = ();

    /// Returns xbase price in relation to base for pool with corresponding `pool_id`.
    fn get_xbase_virtual_price(
        _pool_info: &Self::XdotPoolInfo,
        _ttm: Option<u64>,
    ) -> Result<PriceNumber, DispatchError> {
        Err(DispatchError::Other("XbasePrice"))
    }

    fn get_lp_virtual_price(
        _pool_info: &Self::XdotPoolInfo,
        _ttm: Option<u64>,
    ) -> Result<PriceNumber, DispatchError> {
        Err(DispatchError::Other("XbasePrice"))
    }

    fn get_pool(
        _pool_id: eq_primitives::xdot_pool::PoolId,
    ) -> Result<Self::XdotPoolInfo, DispatchError> {
        Err(DispatchError::Other("XbasePrice"))
    }
}

pub struct OrderAggregatesMock;
impl<AccountId> eq_primitives::dex::OrderAggregates<AccountId> for OrderAggregatesMock {
    fn get_asset_weights(_account_id: &AccountId) -> VecMap<Asset, OrderAggregateBySide> {
        VecMap::new()
    }
}

pub struct FinancialMock<Asset, Price, AccountId>(PhantomData<(Asset, Price, AccountId)>);
impl<Asset, Price: Default, AccountId> financial_pallet::Financial
    for FinancialMock<Asset, Price, AccountId>
{
    type Asset = Asset;
    type Price = Price;
    type AccountId = AccountId;
    fn calc_return(
        _return_type: CalcReturnType,
        _asset: Self::Asset,
    ) -> Result<Vec<Self::Price>, DispatchError> {
        Ok(Default::default())
    }
    fn calc_vol(
        _return_type: CalcReturnType,
        _volatility_type: CalcVolatilityType,
        _asset: Self::Asset,
    ) -> Result<Self::Price, DispatchError> {
        Ok(Default::default())
    }
    fn calc_corr(
        _return_type: CalcReturnType,
        _correlation_type: CalcVolatilityType,
        _asset1: Self::Asset,
        _asset2: Self::Asset,
    ) -> Result<(Price, core::ops::Range<financial_pallet::Duration>), sp_runtime::DispatchError>
    {
        Ok((Default::default(), Default::default()))
    }
    fn calc_portf_vol(
        _return_type: CalcReturnType,
        _vol_cor_type: CalcVolatilityType,
        _account_id: Self::AccountId,
    ) -> Result<Self::Price, DispatchError> {
        Ok(Default::default())
    }
    fn calc_portf_var(
        _return_type: CalcReturnType,
        _vol_cor_type: CalcVolatilityType,
        _account_id: Self::AccountId,
        _z_score: u32,
    ) -> Result<Self::Price, DispatchError> {
        Ok(Default::default())
    }
    fn calc_rv(
        _return_type: CalcReturnType,
        _ewma_length: u32,
        _asset: Self::Asset,
    ) -> Result<Self::Price, DispatchError> {
        Ok(Default::default())
    }
}
