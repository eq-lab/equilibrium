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

//! Module for Bailsman pallet interest rate calculations

use eq_primitives::asset::Asset;
use eq_primitives::balance_number::EqFixedU128;
use eq_utils::{eq_ensure, fixed::fixedi128_from_eq_fixedu128, math::MathUtils};
use financial_pallet::FinancialMetrics;
#[allow(unused_imports)]
use frame_support::{debug, ensure};
use sp_arithmetic::traits::{CheckedDiv, Zero};
use sp_arithmetic::{FixedI128, FixedPointNumber};
use sp_runtime::traits::One;
use sp_std::marker::PhantomData;
use sp_std::{cmp, default::Default, fmt::Debug, prelude::*};

/// Rates errors
#[derive(PartialEq, Debug, Clone)]
pub enum InterestRateError {
    ExternalError,
    NoPrices,
    NoFinancial,
    MathError,
    ValueError,
    ZeroDebt,
    LastUpdateInFuture,
    Overflow,
}

/// Pallet settings object. Settings are stored in pallet's [`Trait`](../trait.Trait.html)
pub struct InterestRateSettings {
    lower_bound: FixedI128,
    upper_bound: FixedI128,
    n_sigma: FixedI128,
    alpha: FixedI128,
}

impl InterestRateSettings {
    pub fn new(
        lower_bound: FixedI128,
        upper_bound: FixedI128,
        n_sigma: FixedI128,
        alpha: FixedI128,
    ) -> InterestRateSettings {
        InterestRateSettings {
            lower_bound,
            upper_bound,
            n_sigma,
            alpha,
        }
    }
}

/// Interface for receiving data required for Bailsman pallet fee calculations
pub trait InterestRateDataSource {
    type AccountId: Debug;
    type Price;

    /// Gets bailsman fee settings
    fn get_settings() -> InterestRateSettings;

    /// Gets `asset` price from oracle
    fn get_price(asset: Asset) -> Result<EqFixedU128, sp_runtime::DispatchError>;

    /// Get `asset` discount
    fn get_discount(asset: Asset) -> EqFixedU128;

    /// Get financial metrics
    fn get_fin_metrics() -> Option<FinancialMetrics<Asset, Self::Price>>;

    /// Get covariance between assets
    fn get_covariance(
        c1: Asset,
        c2: Asset,
        metrics: &FinancialMetrics<Asset, Self::Price>,
    ) -> Option<FixedI128>;

    /// Gets aggregated USD value of collateral and debt for all bailsmen
    fn get_bailsmen_total_balance(asset: Asset) -> TotalBalance;

    /// Gets `SignedBalance` for `account_id` in given `asset` and converts
    /// it into `TotalBalance` used for calculations in Bailsman Pallet
    fn get_balance(account_id: &Self::AccountId, asset: Asset) -> TotalBalance;

    /// Gets `TotalAggregates` for borrowers subaccounts and converts it into
    /// `TotalBalance` used for calculations in Bailsman Pallet
    fn get_borrowers_balance(asset: Asset) -> TotalBalance;
}

/// Struct for storing and transferring associated collateral, debt and
/// bail values
#[derive(PartialEq, Debug, Default)]
pub struct Cdb<T> {
    pub collateral: T,
    pub debt: T,
    pub bail: T,
}

pub type TotalsByCurrency = Totals<Vec<EqFixedU128>>;
pub type TotalsUsd = Totals<EqFixedU128>;
pub type DiscountedTotalsUsd = Totals<FixedI128>;
pub type TotalWeights = Cdb<Vec<FixedI128>>;
pub type TotalInterim = Cdb<Vec<FixedI128>>;
pub type TotalBalance = Cd<EqFixedU128>;

#[derive(PartialEq, Debug, Default)]
pub struct Totals<T: Default> {
    pub borrower: Cd<T>,
    pub bailsman: Cd<T>,
}

#[derive(PartialEq, Debug, Default)]
pub struct Cd<T: Default> {
    pub collateral: T,
    pub debt: T,
}

impl<T: Default> Cd<T> {
    pub fn new(collateral: T, debt: T) -> Self {
        Self { collateral, debt }
    }

    pub fn collateral(value: T) -> Self {
        Self::new(value, T::default())
    }

    pub fn debt(value: T) -> Self {
        Self::new(T::default(), value)
    }
}

/// Multiplies values it 2-member tuples iterator and returns their sum
pub fn sumproduct<'a, I>(items: I) -> FixedI128
where
    I: Iterator<Item = (&'a FixedI128, &'a FixedI128)>,
{
    items.fold(FixedI128::zero(), |acc, (&x, &y)| acc + x * y)
}

/// Service function for volatility calculations
pub fn total_weights(
    xs: &[FixedI128],
    prices: &[FixedI128],
    total: FixedI128,
) -> Result<Vec<FixedI128>, InterestRateError> {
    eq_ensure!(
        total != FixedI128::zero(),
        InterestRateError::ValueError,
        "{}:{}. Total is equal to zero.",
        file!(),
        line!()
    );
    Ok(xs
        .into_iter()
        .zip(prices.into_iter())
        .map(|(&x, &p)| (x * p) / total)
        .collect())
}

/// Calculate weights for borrower(collateral, debt) and bailsman(collateral)
/// w(i) = amount(i) * price / total
pub fn total_weights_aggregated(
    currency_totals: &TotalsByCurrency,
    totals: &TotalsUsd,
    prices: &[EqFixedU128],
) -> Result<TotalWeights, InterestRateError> {
    eq_ensure!(
        totals.borrower.collateral != EqFixedU128::zero(),
        InterestRateError::ValueError,
        "{}:{}. Total borrower collateral is equal to zero.",
        file!(),
        line!(),
    );

    eq_ensure!(
        totals.borrower.debt != EqFixedU128::zero(),
        InterestRateError::ValueError,
        "{}:{}. Total borrower debt is equal to zero.",
        file!(),
        line!(),
    );

    eq_ensure!(
        totals.bailsman.collateral != EqFixedU128::zero(),
        InterestRateError::ValueError,
        "{}:{}. Total bailsman collateral is equal to zero.",
        file!(),
        line!()
    );

    let mut collateral_weights = Vec::with_capacity(prices.len());
    let mut debt_weights = Vec::with_capacity(prices.len());
    let mut bails_weights = Vec::with_capacity(prices.len());

    for i in 0..prices.len() {
        let price = prices[i];

        let collateral_weight = fixedi128_from_eq_fixedu128(
            currency_totals.borrower.collateral[i] * price / totals.borrower.collateral,
        )
        .ok_or_else(|| InterestRateError::Overflow)?;
        collateral_weights.push(collateral_weight);

        let debt_weight = fixedi128_from_eq_fixedu128(
            currency_totals.borrower.debt[i] * price / totals.borrower.debt,
        )
        .ok_or_else(|| InterestRateError::Overflow)?;
        debt_weights.push(debt_weight);

        let bails_weight = fixedi128_from_eq_fixedu128(
            currency_totals.bailsman.collateral[i] * price / totals.bailsman.collateral,
        )
        .ok_or_else(|| InterestRateError::Overflow)?;
        bails_weights.push(bails_weight);
    }

    Ok(Cdb {
        collateral: collateral_weights,
        debt: debt_weights,
        bail: bails_weights,
    })
}

/// Service function for volatility calculations
pub fn total_interim(
    weights: &Vec<FixedI128>,
    covariance_matrix: &Vec<Vec<FixedI128>>,
) -> Vec<FixedI128> {
    covariance_matrix
        .into_iter()
        .map(|covs| sumproduct(covs.into_iter().zip(weights.into_iter())))
        .collect()
}

/// Interim calculations for volatility
pub fn total_interim_aggregated(
    weights: &TotalWeights,
    covariance_matrix: &Vec<Vec<FixedI128>>,
) -> TotalInterim {
    covariance_matrix.into_iter().fold(
        TotalInterim {
            collateral: Vec::with_capacity(covariance_matrix.len()),
            debt: Vec::with_capacity(covariance_matrix.len()),
            bail: Vec::with_capacity(covariance_matrix.len()),
        },
        |mut acc, covs| {
            let (interim_collateral, interim_debt, interim_bails) = covs.iter().enumerate().fold(
                (FixedI128::zero(), FixedI128::zero(), FixedI128::zero()),
                |(coll, debt, bail), (i, &cov)| {
                    (
                        coll + cov * weights.collateral[i],
                        debt + cov * weights.debt[i],
                        bail + cov * weights.bail[i],
                    )
                },
            );

            acc.collateral.push(interim_collateral);
            acc.debt.push(interim_debt);
            acc.bail.push(interim_bails);

            acc
        },
    )
}

/// Aggregate collaterals and debts by currencies for Borrower and Bailsman with collateral discount
/// Calculate total weight for every currency without collateral discount
pub fn totals<T: InterestRateDataSource>(
    currencies: &[Asset],
    prices: &[EqFixedU128],
    collateral_discounts: &[EqFixedU128],
) -> Result<(Totals<FixedI128>, TotalWeights), InterestRateError> {
    let mut currency_totals = TotalsByCurrency {
        borrower: Cd {
            collateral: Vec::with_capacity(currencies.len()),
            debt: Vec::with_capacity(currencies.len()),
        },
        bailsman: Cd {
            collateral: Vec::with_capacity(currencies.len()),
            debt: Vec::with_capacity(currencies.len()),
        },
    };

    let mut discounted_totals_usd: Totals<FixedI128> = Default::default();
    let mut totals_usd: TotalsUsd = Default::default();

    for ((&a, &price), &discount) in currencies.iter().zip(prices).zip(collateral_discounts) {
        let bailsman = T::get_bailsmen_total_balance(a);
        let borrower = T::get_borrowers_balance(a);

        currency_totals
            .bailsman
            .collateral
            .push(bailsman.collateral);
        currency_totals.bailsman.debt.push(bailsman.debt);
        currency_totals
            .borrower
            .collateral
            .push(borrower.collateral);
        currency_totals.borrower.debt.push(borrower.debt);

        //with discount
        {
            let curr_bailsman_collateral =
                fixedi128_from_eq_fixedu128(bailsman.collateral * price * discount)
                    .ok_or_else(|| InterestRateError::Overflow)?;
            discounted_totals_usd.bailsman.collateral =
                discounted_totals_usd.bailsman.collateral + curr_bailsman_collateral;

            let curr_bailsman_debt = fixedi128_from_eq_fixedu128(bailsman.debt * price)
                .ok_or_else(|| InterestRateError::Overflow)?;
            discounted_totals_usd.bailsman.debt =
                discounted_totals_usd.bailsman.debt + curr_bailsman_debt;

            let curr_borrower_coll =
                fixedi128_from_eq_fixedu128(borrower.collateral * price * discount)
                    .ok_or_else(|| InterestRateError::Overflow)?;
            discounted_totals_usd.borrower.collateral =
                discounted_totals_usd.borrower.collateral + curr_borrower_coll;

            let curr_borrower_debt = fixedi128_from_eq_fixedu128(borrower.debt * price)
                .ok_or_else(|| InterestRateError::Overflow)?;
            discounted_totals_usd.borrower.debt =
                discounted_totals_usd.borrower.debt + curr_borrower_debt;
        }

        // without discount
        totals_usd.bailsman.collateral =
            totals_usd.bailsman.collateral + bailsman.collateral * price;
        totals_usd.bailsman.debt = totals_usd.bailsman.debt + bailsman.debt * price;
        totals_usd.borrower.collateral =
            totals_usd.borrower.collateral + borrower.collateral * price;
        totals_usd.borrower.debt = totals_usd.borrower.debt + borrower.debt * price;
    }

    let total_weights = total_weights_aggregated(&currency_totals, &totals_usd, prices)?;

    Ok((discounted_totals_usd, total_weights))
}

/// Calculates bailsman and collateral pools volatilities
pub fn aggregate_portfolio_volatilities(
    total_weights: &TotalWeights,
    covariance_matrix: &Vec<Vec<FixedI128>>,
) -> Result<Cdb<FixedI128>, InterestRateError> {
    let Cdb {
        collateral: collateral_interim,
        debt: debt_interim,
        bail: bail_interim,
    } = total_interim_aggregated(&total_weights, covariance_matrix);

    let mut volatility = Cdb::<_> {
        collateral: FixedI128::zero(),
        debt: FixedI128::zero(),
        bail: FixedI128::zero(),
    };

    for i in 0..total_weights.collateral.len() {
        volatility.collateral =
            volatility.collateral + total_weights.collateral[i] * collateral_interim[i];
        volatility.bail = volatility.bail + total_weights.bail[i] * bail_interim[i];
        volatility.debt = volatility.debt + total_weights.debt[i] * debt_interim[i];
    }

    volatility.collateral = MathUtils::sqrt(volatility.collateral).map_err(|_| {
        log::error!("{}:{}", file!(), line!());
        InterestRateError::MathError
    })?;

    volatility.bail = MathUtils::sqrt(volatility.bail).map_err(|_| {
        log::error!("{}:{}", file!(), line!());
        InterestRateError::MathError
    })?;

    volatility.debt = MathUtils::sqrt(volatility.debt).map_err(|_| {
        log::error!("{}:{}", file!(), line!());
        InterestRateError::MathError
    })?;

    Ok(volatility)
}

/// Gets all covariance values for `asset` from Volatility Pallet
fn covariance_column<T: InterestRateDataSource>(
    asset: Asset,
    assets: &[Asset],
    metrics: &FinancialMetrics<Asset, T::Price>,
) -> Option<Vec<FixedI128>> {
    assets
        .into_iter()
        .map(|&c| T::get_covariance(asset, c, metrics))
        .collect()
}

/// Initialize covariance matrix
pub fn covariance_matrix<T: InterestRateDataSource>(
    currencies: &[Asset],
    metrics: &FinancialMetrics<Asset, T::Price>,
) -> Result<Vec<Vec<FixedI128>>, InterestRateError> {
    currencies
        .iter()
        .map(|&c1| covariance_column::<T>(c1, currencies, metrics))
        .collect::<Option<Vec<Vec<FixedI128>>>>()
        .ok_or_else(|| InterestRateError::NoFinancial)
}

/// System risk model calculations. Calculation with discounts
/// scale = max(min(1/solvency, upper_bound), lower_bound)
/// upper_bound, lower_bound -  from `settings`
pub fn scale(
    discounted_totals: &DiscountedTotalsUsd,
    total_weights: &TotalWeights,
    covariance_matrix: &Vec<Vec<FixedI128>>,
    settings: &InterestRateSettings,
) -> Result<FixedI128, InterestRateError> {
    let total_volatilities = aggregate_portfolio_volatilities(&total_weights, covariance_matrix)?;

    let insufficient_collateral = {
        let collateral_discount = settings.n_sigma * total_volatilities.collateral;
        let debt_discount = settings.n_sigma * total_volatilities.debt;

        cmp::max(
            FixedI128::zero(),
            discounted_totals.borrower.debt * (FixedI128::one() + debt_discount)
                - discounted_totals.borrower.collateral * (FixedI128::one() - collateral_discount),
        )
    };

    let stressed_bail = {
        let bail_discount = settings.n_sigma * total_volatilities.bail;
        (discounted_totals.bailsman.collateral - discounted_totals.bailsman.debt)
            * (FixedI128::one() - bail_discount)
    };

    let stressed_funds = stressed_bail - insufficient_collateral;

    let solvency = discounted_totals
        .bailsman
        .collateral
        .checked_div(&(discounted_totals.bailsman.collateral - stressed_funds))
        .ok_or_else(|| InterestRateError::MathError)?;

    let scale = FixedI128::from(1)
        .checked_div(&solvency)
        .unwrap_or(settings.upper_bound)
        .clamp(settings.lower_bound, settings.upper_bound);
    Ok(scale)
}
/// Calculates leverage L = C / (C - D) where C - user collateral with discount, D - user debt
///
pub fn leverage(
    account_balances: &[TotalBalance],
    prices: &[EqFixedU128],
    discounts: &[EqFixedU128],
) -> Result<FixedI128, InterestRateError> {
    let (discounted_collaterals, debts) = prices.iter().enumerate().try_fold(
        (FixedI128::zero(), FixedI128::zero()),
        |(coll, debt), (i, &price)| {
            let curr_collat =
                fixedi128_from_eq_fixedu128(account_balances[i].collateral * price * discounts[i])
                    .ok_or(InterestRateError::Overflow)?;
            let curr_debt = fixedi128_from_eq_fixedu128(account_balances[i].debt * price)
                .ok_or(InterestRateError::Overflow)?;

            Ok((coll + curr_collat, debt + curr_debt))
        },
    )?;

    discounted_collaterals
        .checked_div(&(discounted_collaterals - debts))
        .ok_or(InterestRateError::MathError)
}

/// Calculates volatility for `positive_balances` - given set of collateral values
pub fn borrower_volatility(
    prices: &Vec<EqFixedU128>,
    account_balances: &Vec<TotalBalance>,
    covariance_matrix: &Vec<Vec<FixedI128>>,
) -> Result<FixedI128, InterestRateError> {
    let (balances, total, prices_fi128) = {
        let mut balances = Vec::with_capacity(prices.len());
        let mut total = FixedI128::zero();
        let mut prices_fi128 = Vec::with_capacity(prices.len());

        for i in 0..prices.len() {
            let price =
                fixedi128_from_eq_fixedu128(prices[i]).ok_or(InterestRateError::Overflow)?;

            prices_fi128.push(price);

            let balance = {
                let coll = fixedi128_from_eq_fixedu128(account_balances[i].collateral)
                    .ok_or(InterestRateError::Overflow)?;
                let debt = fixedi128_from_eq_fixedu128(account_balances[i].debt)
                    .ok_or(InterestRateError::Overflow)?;

                coll - debt
            };

            balances.push(balance);
            total = total + (price * balance).saturating_abs();
        }

        (balances, total, prices_fi128)
    };

    let weights = total_weights(&balances, &prices_fi128, total)?;
    let interim = total_interim(&weights, covariance_matrix);

    let volatility = MathUtils::sqrt(sumproduct(
        (&weights).into_iter().zip((&interim).into_iter()),
    ))
    .map_err(|_| {
        log::error!("{}:{}", file!(), line!());
        InterestRateError::MathError
    })?;

    Ok(volatility)
}

pub struct InterestRateCalculator<'a, T: InterestRateDataSource> {
    _marker: PhantomData<T>, // for saving T
    currencies: &'a [Asset],
    account_balances: Vec<TotalBalance>,
    prices: Vec<EqFixedU128>,
    collateral_discounts: Vec<EqFixedU128>,
}

impl<'a, T: InterestRateDataSource> InterestRateCalculator<'a, T> {
    pub fn create(
        account_id: &'a T::AccountId,
        currencies: &'a [Asset],
    ) -> Result<Self, InterestRateError> {
        let mut account_balances = Vec::with_capacity(currencies.len());
        let mut prices = Vec::with_capacity(currencies.len());
        let mut collateral_discounts = Vec::with_capacity(currencies.len());

        for &currency in currencies {
            account_balances.push(T::get_balance(account_id, currency));
            let price = T::get_price(currency).map_err(|e| {
                log::error!("{}:{}. Unable to fetch price: {:?}", file!(), line!(), e);
                InterestRateError::NoPrices
            })?;
            prices.push(price);

            let discount = T::get_discount(currency);
            collateral_discounts.push(discount);
        }

        Ok(InterestRateCalculator {
            _marker: PhantomData::<T>,
            currencies,
            account_balances,
            prices,
            collateral_discounts,
        })
    }

    /// Calculates prime rate for `account_id`
    /// prime_rate = alpha * L * (vola * scale)^2
    pub fn interest_rate(&self) -> Result<FixedI128, InterestRateError> {
        let settings = T::get_settings();
        let fin_metrics = T::get_fin_metrics().ok_or(InterestRateError::NoFinancial)?;
        let covariance_matrix = covariance_matrix::<T>(&self.currencies, &fin_metrics)?;

        let (discounted_totals, total_weights) =
            totals::<T>(&self.currencies, &self.prices, &self.collateral_discounts)?;

        let scale = scale(
            &discounted_totals,
            &total_weights,
            &covariance_matrix,
            &settings,
        )?;
        let leverage = leverage(
            &self.account_balances,
            &self.prices,
            &self.collateral_discounts,
        )?;

        let vola = borrower_volatility(&self.prices, &self.account_balances, &covariance_matrix)?;

        let interest_rate = settings.alpha * leverage * (scale * vola).sqr();

        log::trace!(
            target: "eq_rate",
            "interest_rate({:?}) = alpha({:?}) * leverage({:?}) * (scale({:?}) * sigma({:?}))^2",
            interest_rate,
            settings.alpha,
            leverage,
            scale,
            vola
        );

        Ok(interest_rate)
    }

    /// Calculate user debt weights for every asset
    /// W(i) = D(i) * P(i) / SUM(D(i) * P(i))
    /// where D(i) - debt of asset 'i', P(i) - price of asset 'i'
    pub fn debt_weights(&self) -> Result<Vec<FixedI128>, InterestRateError> {
        let total_debt = self.account_balances.iter().zip(&self.prices).try_fold(
            FixedI128::zero(),
            |acc, (b, &p)| {
                let curr_debt =
                    fixedi128_from_eq_fixedu128(b.debt * p).ok_or(InterestRateError::Overflow)?;

                Ok(acc + curr_debt)
            },
        )?;

        if total_debt.is_zero() {
            return Err(InterestRateError::ZeroDebt);
        }

        let account_debts = self
            .account_balances
            .iter()
            .map(|b| fixedi128_from_eq_fixedu128(b.debt).ok_or(InterestRateError::Overflow))
            .collect::<Result<Vec<_>, _>>()?;

        let prices = self
            .prices
            .iter()
            .map(|p| fixedi128_from_eq_fixedu128(*p).ok_or(InterestRateError::Overflow))
            .collect::<Result<Vec<_>, _>>()?;

        total_weights(&account_debts, &prices, total_debt)
    }
}
