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

use crate::rate::{
    aggregate_portfolio_volatilities, borrower_volatility, leverage, scale, total_interim,
    total_interim_aggregated, total_weights, totals, InterestRateDataSource, InterestRateError,
    InterestRateSettings, TotalBalance,
};
use financial_pallet::FinancialMetrics;

use eq_primitives::asset;
use eq_primitives::asset::Asset;
use eq_utils::{assert_eq_fx128, fx128};
use sp_arithmetic::{FixedI128, FixedPointNumber};

use crate::InterestRateCalculator;
use eq_primitives::balance_number::EqFixedU128;
use frame_support::assert_err;
use sp_runtime::traits::{One, Zero};
use sp_runtime::DispatchError;

pub fn prices<T: InterestRateDataSource>(currencies: &[Asset]) -> Vec<EqFixedU128> {
    currencies
        .into_iter()
        .map(|x| T::get_price(*x).ok().unwrap())
        .collect()
}

pub fn discounts<T: InterestRateDataSource>(currencies: &[Asset]) -> Vec<EqFixedU128> {
    currencies.iter().map(|&c| T::get_discount(c)).collect()
}

fn balances<T: InterestRateDataSource>(
    account_id: &T::AccountId,
    currencies: &[Asset],
) -> Vec<TotalBalance> {
    currencies
        .iter()
        .map(|&x| T::get_balance(account_id, x))
        .collect()
}

fn high_volatility_covariance_matrix() -> Vec<Vec<FixedI128>> {
    vec![
        vec![
            fx128!(0, 023000),
            fx128!(0, 020000),
            fx128!(0, 025000),
            fx128!(0, 000000),
            fx128!(0, 020000),
        ],
        vec![
            fx128!(0, 020000),
            fx128!(0, 028600),
            fx128!(0, 017000),
            fx128!(0, 000000),
            fx128!(0, 014500),
        ],
        vec![
            fx128!(0, 025000),
            fx128!(0, 017000),
            fx128!(0, 018000),
            fx128!(0, 000000),
            fx128!(0, 027000),
        ],
        vec![
            fx128!(0, 000000),
            fx128!(0, 000000),
            fx128!(0, 000000),
            fx128!(0, 000000),
            fx128!(0, 000000),
        ],
        vec![
            fx128!(0, 020000),
            fx128!(0, 014500),
            fx128!(0, 027000),
            fx128!(0, 000000),
            fx128!(0, 012000),
        ],
    ]
}

fn covariance_matrix() -> Vec<Vec<FixedI128>> {
    vec![
        vec![
            fx128!(0, 0064),
            fx128!(0, 0064),
            fx128!(0, 0),
            fx128!(0, 00288),
            fx128!(0, 0),
        ],
        vec![
            fx128!(0, 0064),
            fx128!(0, 01),
            fx128!(0, 0),
            fx128!(0, 003),
            fx128!(0, 0),
        ],
        vec![
            fx128!(0, 0),
            fx128!(0, 0),
            fx128!(0, 00000001),
            fx128!(0, 0000006).neg(),
            fx128!(0, 00000002),
        ],
        vec![
            fx128!(0, 00288),
            fx128!(0, 003),
            fx128!(0, 0000006).neg(),
            fx128!(0, 0036),
            fx128!(0, 0),
        ],
        vec![
            fx128!(0, 0),
            fx128!(0, 0),
            fx128!(0, 00000002),
            fx128!(0, 0),
            fx128!(0, 00000004),
        ],
    ]
}

struct TestSource;

impl InterestRateDataSource for TestSource {
    type AccountId = ();
    type Price = ();

    fn get_settings() -> InterestRateSettings {
        let lover_bound = FixedI128::saturating_from_rational(1, 2);
        let upper_bound = FixedI128::saturating_from_integer(2);
        let n_sigma = FixedI128::saturating_from_integer(5);
        let alpha = FixedI128::saturating_from_integer(15);

        InterestRateSettings::new(lover_bound, upper_bound, n_sigma, alpha)
    }

    fn get_price(asset: Asset) -> Result<EqFixedU128, DispatchError> {
        let price = match asset {
            asset::DOT => EqFixedU128::from(7),
            asset::EQ => EqFixedU128::saturating_from_rational(5, 10),
            asset::USDT => EqFixedU128::from(1),
            asset::ETH => EqFixedU128::from(1200),
            asset::EQD => EqFixedU128::from(1),
            _ => EqFixedU128::one(),
        };

        Ok(price)
    }

    fn get_discount(asset: Asset) -> EqFixedU128 {
        match asset {
            asset::DOT => EqFixedU128::saturating_from_rational(75, 100),
            asset::EQ => EqFixedU128::saturating_from_rational(5, 10),
            asset::USDT => EqFixedU128::from(1),
            asset::ETH => EqFixedU128::saturating_from_rational(9, 10),
            asset::EQD => EqFixedU128::from(1),
            _ => EqFixedU128::from(1),
        }
    }

    fn get_fin_metrics() -> Option<FinancialMetrics<Asset, Self::Price>> {
        Some(Default::default())
    }

    fn get_covariance(
        c1: Asset,
        c2: Asset,
        _metrics: &FinancialMetrics<Asset, Self::Price>,
    ) -> Option<FixedI128> {
        let covariance = match (c1, c2) {
            (asset::DOT, asset::DOT) => fx128!(0, 0064),
            (asset::DOT, asset::EQ) => fx128!(0, 0064),
            (asset::DOT, asset::USDT) => fx128!(0, 0),
            (asset::DOT, asset::ETH) => fx128!(0, 00288),
            (asset::DOT, asset::EQD) => fx128!(0, 0),

            (asset::EQ, asset::DOT) => fx128!(0, 0064),
            (asset::EQ, asset::EQ) => fx128!(0, 01),
            (asset::EQ, asset::USDT) => fx128!(0, 0),
            (asset::EQ, asset::ETH) => fx128!(0, 003),
            (asset::EQ, asset::EQD) => fx128!(0, 0),

            (asset::USDT, asset::DOT) => fx128!(0, 0),
            (asset::USDT, asset::EQ) => fx128!(0, 0),
            (asset::USDT, asset::USDT) => fx128!(0, 00000001),
            (asset::USDT, asset::ETH) => fx128!(0, 0000006).neg(),
            (asset::USDT, asset::EQD) => fx128!(0, 00000002),

            (asset::ETH, asset::DOT) => fx128!(0, 00288),
            (asset::ETH, asset::EQ) => fx128!(0, 003),
            (asset::ETH, asset::USDT) => fx128!(0, 0000006).neg(),
            (asset::ETH, asset::ETH) => fx128!(0, 0036),
            (asset::ETH, asset::EQD) => fx128!(0, 0),

            (asset::EQD, asset::DOT) => fx128!(0, 0),
            (asset::EQD, asset::EQ) => fx128!(0, 0),
            (asset::EQD, asset::USDT) => fx128!(0, 00000002),
            (asset::EQD, asset::ETH) => fx128!(0, 0),
            (asset::EQD, asset::EQD) => fx128!(0, 00000004),
            _ => panic!("Wrong test settings for covariance"),
        };

        Some(covariance)
    }

    fn get_bailsmen_total_balance(asset: Asset) -> TotalBalance {
        match asset {
            asset::ETH => TotalBalance::collateral(EqFixedU128::from(10)),
            _ => TotalBalance::new(EqFixedU128::zero(), EqFixedU128::zero()),
        }
    }

    fn get_balance(_account_id: &Self::AccountId, asset: Asset) -> TotalBalance {
        match asset {
            asset::DOT => TotalBalance::collateral(EqFixedU128::from(1_000)),
            asset::EQ => TotalBalance::collateral(EqFixedU128::from(100_000)),
            asset::USDT => TotalBalance::collateral(EqFixedU128::from(10_000)),
            asset::ETH => TotalBalance::collateral(EqFixedU128::from(20)),
            asset::EQD => TotalBalance::debt(EqFixedU128::from(50_000)),
            _ => TotalBalance::new(EqFixedU128::from(0), EqFixedU128::from(0)),
        }
    }

    fn get_borrowers_balance(asset: Asset) -> TotalBalance {
        match asset {
            asset::DOT => TotalBalance::collateral(EqFixedU128::from(1_000)),
            asset::EQ => TotalBalance::collateral(EqFixedU128::from(100_000)),
            asset::USDT => TotalBalance::collateral(EqFixedU128::from(10_000)),
            asset::ETH => TotalBalance::collateral(EqFixedU128::from(20)),
            asset::EQD => TotalBalance::debt(EqFixedU128::from(50_000)),
            _ => TotalBalance::new(EqFixedU128::from(0), EqFixedU128::from(0)),
        }
    }
}

#[test]
fn totals_test() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let prices = prices::<TestSource>(&currencies);
    let discounts = discounts::<TestSource>(&currencies);

    let (totals, total_weights) = totals::<TestSource>(&currencies, &prices, &discounts).unwrap();

    assert_eq!(totals.borrower.collateral, fx128!(61_850, 0));
    assert_eq!(totals.borrower.debt, fx128!(50_000, 0));
    assert_eq!(totals.bailsman.collateral, fx128!(10_800, 0));
    assert_eq!(totals.bailsman.debt, fx128!(0, 0));

    assert_eq_fx128!(total_weights.collateral[0], fx128!(0, 0769), 4);
    assert_eq_fx128!(total_weights.collateral[1], fx128!(0, 5495), 4);
    assert_eq_fx128!(total_weights.collateral[2], fx128!(0, 1099), 4);
    assert_eq_fx128!(total_weights.collateral[3], fx128!(0, 2637), 4);
    assert_eq_fx128!(total_weights.collateral[4], fx128!(0, 0), 4);

    assert_eq_fx128!(total_weights.debt[0], fx128!(0, 0), 4);
    assert_eq_fx128!(total_weights.debt[1], fx128!(0, 0), 4);
    assert_eq_fx128!(total_weights.debt[2], fx128!(0, 0), 4);
    assert_eq_fx128!(total_weights.debt[3], fx128!(0, 0), 4);
    assert_eq_fx128!(total_weights.debt[4], fx128!(1, 0), 4);

    assert_eq_fx128!(total_weights.bail[0], fx128!(0, 0), 4);
    assert_eq_fx128!(total_weights.bail[1], fx128!(0, 0), 4);
    assert_eq_fx128!(total_weights.bail[2], fx128!(0, 0), 4);
    assert_eq_fx128!(total_weights.bail[3], fx128!(1, 0), 4);
    assert_eq_fx128!(total_weights.bail[4], fx128!(0, 0), 4);
}

#[test]
fn aggregate_portfolio_volatilities_test() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let prices = prices::<TestSource>(&currencies);
    let discounts = discounts::<TestSource>(&currencies);

    let (_, total_weights) = totals::<TestSource>(&currencies, &prices, &discounts).unwrap();

    let total_volatilities =
        aggregate_portfolio_volatilities(&total_weights, &covariance_matrix()).unwrap();

    assert_eq_fx128!(total_volatilities.collateral, fx128!(0, 0695), 4);
    assert_eq_fx128!(total_volatilities.debt, fx128!(0, 000199), 4);
    assert_eq_fx128!(total_volatilities.bail, fx128!(0, 06), 4);
}

#[test]
fn test_scale() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let prices = prices::<TestSource>(&currencies);
    let discounts = discounts::<TestSource>(&currencies);
    let settings = TestSource::get_settings();

    let (totals, total_weights) = totals::<TestSource>(&currencies, &prices, &discounts).unwrap();

    let scale = scale(&totals, &total_weights, &covariance_matrix(), &settings).unwrap();

    assert_eq_fx128!(fx128!(1, 19), scale, 2);
}

#[test]
fn test_high_volatility_scale() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let prices = prices::<TestSource>(&currencies);
    let discounts = discounts::<TestSource>(&currencies);
    let settings = TestSource::get_settings();

    let (totals, total_weights) = totals::<TestSource>(&currencies, &prices, &discounts).unwrap();

    let scale = scale(
        &totals,
        &total_weights,
        &high_volatility_covariance_matrix(),
        &settings,
    )
    .unwrap();

    assert_eq_fx128!(fx128!(2, 0), scale, 2);
}

#[test]
fn leverage_test() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let prices = prices::<TestSource>(&currencies);
    let discounts = discounts::<TestSource>(&currencies);
    let account_balances = balances::<TestSource>(&(), &currencies);

    let leverage = leverage(&account_balances, &prices, &discounts).unwrap();
    assert_eq_fx128!(leverage, fx128!(5, 2194092), 6);
}

#[test]
fn leverage_should_error_when_zero_collateral() {
    let account_balances = vec![TotalBalance::new(EqFixedU128::one(), EqFixedU128::one())];
    let prices = vec![EqFixedU128::one()];
    let discounts = vec![EqFixedU128::one()];

    assert_err!(
        leverage(&account_balances, &prices, &discounts),
        InterestRateError::MathError
    );
}

#[test]
fn test_borrower_volatility() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let account_balances = balances::<TestSource>(&(), &currencies);
    let prices = prices::<TestSource>(&currencies);

    let covariance_matrix = covariance_matrix();

    let volatility = borrower_volatility(&prices, &account_balances, &covariance_matrix).unwrap();

    println!("{:?}", volatility);
    assert_eq_fx128!(fx128!(0, 04487443878), volatility, 5);
}

#[test]
fn test_total_weights() {
    let prices = vec![
        fx128!(10000, 0),
        fx128!(250, 0),
        fx128!(3, 0),
        fx128!(1, 0),
        fx128!(25, 0),
    ];
    let collateral = vec![
        fx128!(1, 5),
        fx128!(30, 0),
        fx128!(1500, 0),
        fx128!(0, 0),
        fx128!(0, 0),
    ];
    let total = fx128!(27000, 0);

    let actual = total_weights(&collateral, &prices, total).unwrap();

    let expected = vec![
        fx128!(0, 555555555555555555),
        fx128!(0, 277777777777777777),
        fx128!(0, 166666666666666666),
        fx128!(0, 0),
        fx128!(0, 0),
    ];

    assert_eq!(actual, expected);
}

#[test]
fn test_total_interim() {
    let weights = vec![
        fx128!(0, 04964539007),
        fx128!(0, 3546099291),
        fx128!(0, 07092198582),
        fx128!(0, 170212766),
        fx128!(0, 000000000),
    ];
    let covariance_matrix = covariance_matrix();
    let actual = total_interim(&weights, &covariance_matrix);

    let expected = vec![
        fx128!(0, 003077446809),
        fx128!(0, 004374468085),
        fx128!(0, 0000001085106383).neg(),
        fx128!(0, 001819531915),
        fx128!(0, 00000001276595745).neg(),
    ];

    for i in 0..expected.len() {
        assert_eq_fx128!(actual[i], expected[i], 5);
    }
}

#[test]
fn test_total_interim_aggregated() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let prices = prices::<TestSource>(&currencies);
    let discounts = discounts::<TestSource>(&currencies);
    let (_, total_weights) = totals::<TestSource>(&currencies, &prices, &discounts).unwrap();

    let total_interim = total_interim_aggregated(&total_weights, &covariance_matrix());

    let expected = vec![
        fx128!(0, 004768351648),
        fx128!(0, 006778021978),
        fx128!(0, 0000001571428571).neg(),
        fx128!(0, 002819274725),
        fx128!(0, 000000002197802198),
    ];

    for i in 0..expected.len() {
        assert_eq_fx128!(total_interim.collateral[i], expected[i], 5);
    }
}

#[test]
fn test_interest_rate() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let calculator = InterestRateCalculator::<TestSource>::create(&(), &currencies).unwrap();
    let interest_rate = calculator.interest_rate().unwrap();

    let expected_interest_rate = fx128!(0, 226406955);
    assert_eq_fx128!(interest_rate, expected_interest_rate, 9);
}

struct ZeroDebtSource;
impl InterestRateDataSource for ZeroDebtSource {
    type AccountId = ();
    type Price = ();

    fn get_settings() -> InterestRateSettings {
        todo!()
    }

    fn get_price(_asset: Asset) -> Result<EqFixedU128, DispatchError> {
        Ok(EqFixedU128::one())
    }

    fn get_discount(_asset: Asset) -> EqFixedU128 {
        EqFixedU128::one()
    }

    fn get_fin_metrics() -> Option<FinancialMetrics<Asset, Self::Price>> {
        None
    }

    fn get_covariance(
        _c1: Asset,
        _c2: Asset,
        _metrics: &FinancialMetrics<Asset, Self::Price>,
    ) -> Option<FixedI128> {
        None
    }

    fn get_bailsmen_total_balance(_asset: Asset) -> TotalBalance {
        TotalBalance::new(EqFixedU128::one(), EqFixedU128::zero())
    }

    fn get_balance(_account_id: &Self::AccountId, _asset: Asset) -> TotalBalance {
        TotalBalance::new(EqFixedU128::one(), EqFixedU128::zero())
    }

    fn get_borrowers_balance(_asset: Asset) -> TotalBalance {
        TotalBalance::new(EqFixedU128::zero(), EqFixedU128::zero())
    }
}

#[test]
fn debt_weight_should_return_error_when_zero_weight() {
    let currencies = vec![asset::DOT, asset::EQ, asset::USDT, asset::ETH, asset::EQD];
    let calculator = InterestRateCalculator::<ZeroDebtSource>::create(&(), &currencies).unwrap();
    assert_err!(calculator.debt_weights(), InterestRateError::ZeroDebt);
}
