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

use sp_std::marker::PhantomData;

use sp_runtime::traits::Convert;
use substrate_fixed::traits::{Fixed, FixedSigned};
use substrate_fixed::transcendental::pow;
use substrate_fixed::types::{I64F64, I9F23};

// Type of constants for transcendental operations declared in substrate_fixed crate
pub type ConstType = I9F23;
pub type FixedNumberBits = i128;

pub trait YieldMathTrait<Number> {
    /// The number type for underlying calculations
    type Error: sp_std::fmt::Debug;
    /**
     * Calculate a YieldSpace pool invariant according to the whitepaper
     */
    fn invariant(
        base_reserves: Number,
        fy_token_reserves: Number,
        total_supply: Number,
        time_till_maturity: Number,
        ts: Number,
        g2: Number,
        calc_price: bool,
    ) -> Result<Number, Self::Error>;

    /**
     * https://www.desmos.com/calculator/5nf2xuy6yb
     * Calculate the amount of fyToken a user would get for given amount of Base.
     * @param base_reserves base reserves amount
     * @param fy_token_reserves fyToken reserves amount
     * @param base_amount base amount to be traded
     * @param time_till_maturity time till maturity in seconds
     * @param ts time till maturity coefficient, multiplied by 2^64
     * @param g fee coefficient, multiplied by 2^64
     * @return the amount of fyToken a user would get for given amount of Base
     */
    fn fy_token_out_for_base_in(
        base_reserves: Number,
        fy_token_reserves: Number,
        base_amount: Number,
        time_till_maturity: Number,
        ts: Number,
        g: Number,
    ) -> Result<Number, Self::Error>;

    /**
     * Calculate the amount of base a user would get for certain amount of fyToken.
     * https://www.desmos.com/calculator/6jlrre7ybt
     * @param base_reserves base reserves amount
     * @param fy_token_reserves fyToken reserves amount
     * @param fy_token_amount fyToken amount to be traded
     * @param time_till_maturity time till maturity in seconds
     * @param ts time till maturity coefficient, multiplied by 2^64
     * @param g fee coefficient, multiplied by 2^64
     * @return the amount of Base a user would get for given amount of fyToken
     */
    fn base_out_for_fy_token_in(
        base_reserves: Number,
        fy_token_reserves: Number,
        fy_token_amount: Number,
        time_till_maturity: Number,
        ts: Number,
        g: Number,
        calc_price: bool,
    ) -> Result<Number, Self::Error>;

    /**
     * Calculate the amount of fyToken a user could sell for given amount of Base.
     * https://www.desmos.com/calculator/0rgnmtckvy
     * @param base_reserves base reserves amount
     * @param fy_token_reserves fyToken reserves amount
     * @param base_amount Base amount to be traded
     * @param time_till_maturity time till maturity in seconds
     * @param ts time till maturity coefficient, multiplied by 2^64
     * @param g fee coefficient, multiplied by 2^64
     * @return the amount of fyToken a user could sell for given amount of Base
     */
    fn fy_token_in_for_base_out(
        base_reserves: Number,
        fy_token_reserves: Number,
        base_amount: Number,
        time_till_maturity: Number,
        ts: Number,
        g: Number,
    ) -> Result<Number, Self::Error>;

    /**
     * Calculate the amount of base a user would have to pay for certain amount of fyToken.
     * https://www.desmos.com/calculator/ws5oqj8x5i
     * @param base_reserves Base reserves amount
     * @param fy_token_reserves fyToken reserves amount
     * @param fy_token_amount fyToken amount to be traded
     * @param time_till_maturity time till maturity in seconds
     * @param ts time till maturity coefficient, multiplied by 2^64
     * @param g fee coefficient, multiplied by 2^64
     * @return the amount of base a user would have to pay for given amount of
     *         fyToken
     */
    fn base_in_for_fy_token_out(
        base_reserves: Number,
        fy_token_reserves: Number,
        fy_token_amount: Number,
        time_till_maturity: Number,
        ts: Number,
        g: Number,
    ) -> Result<Number, Self::Error>;

    /**
     * Calculate the max amount of fyTokens that can be bought from the pool without making the interest rate negative.
     * See section 6.3 of the YieldSpace White paper
     * @param base_reserves Base reserves amount
     * @param fy_token_reserves fyToken reserves amount
     * @param time_till_maturity time till maturity in seconds
     * @param ts time till maturity coefficient, multiplied by 2^64
     * @param g fee coefficient, multiplied by 2^64
     * @return max amount of fyTokens that can be bought from the pool
     */
    fn max_fy_token_out(
        base_reserves: Number,
        fy_token_reserves: Number,
        time_till_maturity: Number,
        ts: Number,
        g: Number,
    ) -> Result<Number, Self::Error>;

    /**
     * Calculate the max amount of fyTokens that can be sold to into the pool.
     * @param base_reserves Base reserves amount
     * @param fy_token_reserves fyToken reserves amount
     * @param time_till_maturity time till maturity in seconds
     * @param ts time till maturity coefficient, multiplied by 2^64
     * @param g fee coefficient, multiplied by 2^64
     * @return max amount of fyTokens that can be sold to into the pool
     */
    fn max_fy_token_in(
        base_reserves: Number,
        fy_token_reserves: Number,
        time_till_maturity: Number,
        ts: Number,
        g: Number,
    ) -> Result<Number, Self::Error>;

    /**
     * Calculate the max amount of base that can be sold to into the pool without making the interest rate negative.
     * @param base_reserves Base reserves amount
     * @param fy_token_reserves fyToken reserves amount
     * @param time_till_maturity time till maturity in seconds
     * @param ts time till maturity coefficient, multiplied by 2^64
     * @param g fee coefficient, multiplied by 2^64
     * @return max amount of base that can be sold to into the pool
     */
    fn max_base_in(
        base_reserves: Number,
        fy_token_reserves: Number,
        time_till_maturity: Number,
        ts: Number,
        g: Number,
    ) -> Result<Number, Self::Error>;

    /**
     * Calculate the max amount of base that can be bought from the pool.
     * @param base_reserves Base reserves amount
     * @param fy_token_reserves fyToken reserves amount
     * @param time_till_maturity time till maturity in seconds
     * @param ts time till maturity coefficient, multiplied by 2^64
     * @param g fee coefficient, multiplied by 2^64
     * @return max amount of base that can be bought from the pool
     */
    fn max_base_out(
        base_reserves: Number,
        fy_token_reserves: Number,
        time_till_maturity: Number,
        ts: Number,
        g: Number,
    ) -> Result<Number, Self::Error>;

    fn compute_a(
        time_till_maturity: Number,
        ts: Number,
        g: Number,
        calc_price: bool,
    ) -> Result<Number, Self::Error>;

    fn compute_b(time_till_maturity: Number, ts: Number, g: Number) -> Result<Number, Self::Error>;
}

#[derive(Debug, Clone)]
pub enum YieldMathError {
    // Invariant
    InvariantCalcOverflow,
    InvariantCalcPowOverflow,
    // Compute a and b coeff
    ComputeAOverflow,
    ComputeBOverflow,
    NegativeTime,
    TooFarFromMaturity,
    CoeffGMustBePositive,
    // FyTokenOutForBaseIn
    FyTokenOutForBaseInPowOverflow,
    FyTokenOutForBaseInOverflow,
    // BaseOutForFyTokenIn
    BaseOutForFyTokenInOverflow,
    BaseOutForFyTokenInPowOverflow,
    // FyTokenInForBaseOut
    FyTokenInForBaseOutPowOverflow,
    FyTokenInForBaseOutOverflow,
    // BaseInForFyTokenOut
    BaseInForFyTokenOutPowOverflow,
    BaseInForFyTokenOutOverflow,
    // MaxFyTokenOut
    MaxFyTokenOutOverflow,
    MaxFyTokenOutPowOverflow,
    // MaxFyTokenIn
    MaxFyTokenInPowOverflow,
    MaxFyTokenInOverflow,
    // Common
    FyTokenReservesTooLow,
    BaseTokenReservesTooLow,
    TooMuchBaseIn,
    TooMuchFyTokenIn,
    TooMuchFyTokenOut,
}

pub struct YieldConvert;

impl Convert<u64, I64F64> for YieldConvert {
    fn convert(n: u64) -> I64F64 {
        let res = I64F64::from_num(n);
        res
    }
}

pub struct YieldMath<N, Convert>(PhantomData<N>, PhantomData<Convert>);

impl<N, C> YieldMath<N, C>
where
    N: Fixed + FixedSigned<Bits = FixedNumberBits> + PartialOrd<ConstType> + From<ConstType>,
    C: Convert<u64, N>, // + CheckedConvert<usize, Number>
                        // + Convert<Number, Self::Balance>
{
    fn get_number(n: u64) -> N {
        <C as Convert<u64, N>>::convert(n)
    }
}

impl<N, C> YieldMathTrait<N> for YieldMath<N, C>
where
    N: Fixed + FixedSigned<Bits = FixedNumberBits> + PartialOrd<ConstType> + From<ConstType>,
    C: Convert<u64, N>,
{
    type Error = YieldMathError;

    fn invariant(
        base_reserves: N,
        fy_token_reserves: N,
        total_supply: N,
        time_till_maturity: N,
        ts: N,
        g2: N,
        calc_price: bool,
    ) -> Result<N, YieldMathError> {
        let zero = N::from_num(0);
        if total_supply == zero {
            return Ok(zero);
        }

        // in yield v2 is used 10-years annual rate
        // ts = int128(uint128(uint256((1 << 64))) / 315576000); // 1 / Seconds in 10 years, in 64.64

        // a = (1 - ts * time_till_maturity)
        let normalized_time = ts
            .checked_mul(time_till_maturity)
            .ok_or(YieldMathError::InvariantCalcOverflow)?
            .checked_mul(g2)
            .ok_or(YieldMathError::InvariantCalcOverflow)?;
        let one = N::from_num(1);
        let a = one
            .checked_sub(normalized_time)
            .ok_or(YieldMathError::InvariantCalcOverflow)?;

        if !calc_price && (a < zero || a == zero) {
            return Err(YieldMathError::TooFarFromMaturity);
        };

        let base_pow: N =
            pow(base_reserves, a).map_err(|_| YieldMathError::InvariantCalcPowOverflow)?;
        // println!("base_reserves {:?} base_pow {:?}", base_reserves, base_pow);
        let reserve_pow: N =
            pow(fy_token_reserves, a).map_err(|_| YieldMathError::InvariantCalcPowOverflow)?;
        // println!(
        //     "fy_token_reserves {:?} reserve_pow {:?}",
        //     fy_token_reserves, reserve_pow
        // );

        let inv_a = one
            .checked_div(a)
            .ok_or(YieldMathError::InvariantCalcOverflow)?;

        let invariant = base_pow
            .checked_add(reserve_pow)
            .ok_or(YieldMathError::InvariantCalcOverflow)
            .map(|s: N| {
                // println!("s {:?} two {:?}", s, Self::get_number(2));
                let half_s = s / Self::get_number(2);
                // println!("half_s {:?}", half_s);
                pow(half_s, inv_a).map_err(|_| YieldMathError::InvariantCalcPowOverflow)
            })?
            .map(|n: N| n.checked_div(total_supply))?
            .ok_or(YieldMathError::InvariantCalcOverflow);

        invariant
    }

    fn fy_token_out_for_base_in(
        base_reserves: N,
        fy_token_reserves: N,
        base_amount: N,
        time_till_maturity: N,
        ts: N,
        g: N,
    ) -> Result<N, YieldMathError> {
        let a = Self::compute_a(time_till_maturity, ts, g, false)?;
        // za = base_reserves ** a
        let za: N =
            pow(base_reserves, a).map_err(|_| YieldMathError::FyTokenOutForBaseInPowOverflow)?;

        // ya = fy_token_reserves ** a
        let ya = pow(fy_token_reserves, a)
            .map_err(|_| YieldMathError::FyTokenOutForBaseInPowOverflow)?;

        // zx = base_reserves + base_amount
        let zx = base_reserves
            .checked_add(base_amount)
            .ok_or(YieldMathError::TooMuchBaseIn)?;

        // zxa = zx ** a
        let zxa = pow(zx, a).map_err(|_| YieldMathError::FyTokenOutForBaseInPowOverflow)?;

        // sum = za + ya - zxa
        let sum: N = za
            .checked_add(ya)
            .ok_or(YieldMathError::FyTokenOutForBaseInOverflow)?
            .checked_sub(zxa)
            .ok_or(YieldMathError::FyTokenOutForBaseInOverflow)?; // z < MAX, y < MAX, a < 1. It can only underflow, not overflow.

        if sum < N::from_num(0) {
            return Err(YieldMathError::FyTokenReservesTooLow);
        }

        // result = fy_token_reserves - (sum ** (1/a))

        let one = N::from_num(1);
        let inv_a = one
            .checked_div(a)
            .ok_or(YieldMathError::FyTokenOutForBaseInOverflow)?;

        let result = fy_token_reserves.checked_sub(
            // sum.pow(inv_a)
            //     .map_err(|_| YieldMathError::FyTokenOutForBaseInPowOverflow)?,
            pow(sum, inv_a).map_err(|_| YieldMathError::FyTokenOutForBaseInPowOverflow)?,
        );

        result.ok_or(YieldMathError::FyTokenOutForBaseInOverflow)
    }

    fn base_out_for_fy_token_in(
        base_reserves: N,
        fy_token_reserves: N,
        fy_token_amount: N,
        time_till_maturity: N,
        ts: N,
        g: N,
        calc_price: bool,
    ) -> Result<N, YieldMathError> {
        let a = Self::compute_a(time_till_maturity, ts, g, calc_price)?;
        // println!("a {:?}", a);

        // za = base_reserves ** a
        let za: N =
            pow(base_reserves, a).map_err(|_| YieldMathError::BaseOutForFyTokenInPowOverflow)?;
        // println!("za {:?}", za);

        // ya = fy_token_reserves ** a
        let ya: N = pow(fy_token_reserves, a)
            .map_err(|_| YieldMathError::BaseOutForFyTokenInPowOverflow)?;
        // println!("ya {:?}", ya);

        // yx = fyDayReserves + fy_token_amount
        let yx: N = fy_token_reserves
            .checked_add(fy_token_amount)
            .ok_or(YieldMathError::TooMuchFyTokenIn)?;
        // println!("yx {:?}", yx);

        // yxa = yx ** a
        let yxa = pow(yx, a).map_err(|_| YieldMathError::BaseOutForFyTokenInPowOverflow)?;
        // println!("yxa {:?}", yxa);

        // sum = za + ya - yxa
        let sum = za // unsigned
            .checked_add(ya)
            .ok_or(YieldMathError::BaseOutForFyTokenInOverflow)?
            .checked_sub(yxa)
            .ok_or(YieldMathError::BaseOutForFyTokenInOverflow)?; // z < MAX, y < MAX, a < 1. It can only underflow, not overflow.

        if sum < N::from_num(0) {
            return Err(YieldMathError::BaseTokenReservesTooLow);
        }

        // println!("sum {:?}", sum);
        let one = N::from_num(1);
        let inv_a = one
            .checked_div(a)
            .ok_or(YieldMathError::BaseOutForFyTokenInOverflow)?;

        // println!("inv_a {:?}", inv_a);

        // result = base_reserves - (sum ** (1/a))
        // println!("sum pow {:?}", sum.pow(inv_a));
        let sum_pow: N =
            pow(sum, inv_a).map_err(|_| YieldMathError::BaseOutForFyTokenInPowOverflow)?;
        let result = base_reserves.checked_sub(
            // sum.pow(inv_a)
            //     .map_err(|_| YieldMathError::BaseOutForFyTokenInPowOverflow)?;
            sum_pow,
        );

        result.ok_or(YieldMathError::BaseOutForFyTokenInOverflow)
    }

    fn fy_token_in_for_base_out(
        base_reserves: N,
        fy_token_reserves: N,
        base_amount: N,
        time_till_maturity: N,
        ts: N,
        g: N,
    ) -> Result<N, Self::Error> {
        let a = Self::compute_a(time_till_maturity, ts, g, false)?;

        // za = base_reserves ** a
        // let za = base_reserves
        //     .pow(a)
        //     .map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?;
        let za: N =
            pow(base_reserves, a).map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?;

        // ya = fy_token_reserves ** a
        // let ya = fy_token_reserves
        //     .pow(a)
        //     .map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?;
        let ya = pow(fy_token_reserves, a)
            .map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?;

        // zx = base_reserves - base_amount
        let zx = base_reserves
            .checked_sub(base_amount)
            .ok_or(YieldMathError::FyTokenInForBaseOutOverflow)?;

        // zxa = zx ** a
        // let zxa = zx
        //     .pow(a)
        //     .map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?;
        let zxa = pow(zx, a).map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?;

        // sum = za + ya - zxa
        let sum = za
            .checked_add(ya)
            .ok_or(YieldMathError::FyTokenInForBaseOutOverflow)?
            .checked_sub(zxa)
            .ok_or(YieldMathError::FyTokenInForBaseOutOverflow)?; // z < MAX, y < MAX, a < 1. It can only underflow, not overflow.

        if sum < N::from_num(0) {
            return Err(YieldMathError::BaseTokenReservesTooLow);
        }

        let one = N::from_num(1);
        let inv_a = one
            .checked_div(a)
            .ok_or(YieldMathError::FyTokenInForBaseOutOverflow)?;

        // result = (sum ** (1/a)) - fy_token_reserves
        // let result = sum
        //     .pow(inv_a)
        //     .map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?
        //     .checked_sub(fy_token_reserves);

        let sum_pow: N =
            pow(sum, inv_a).map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?;

        let result = sum_pow.checked_sub(fy_token_reserves);

        result.ok_or(YieldMathError::FyTokenInForBaseOutOverflow)
    }

    fn base_in_for_fy_token_out(
        base_reserves: N,
        fy_token_reserves: N,
        fy_token_amount: N,
        time_till_maturity: N,
        ts: N,
        g: N,
    ) -> Result<N, Self::Error> {
        let a = Self::compute_a(time_till_maturity, ts, g, false)?;
        // println!("base_in_for_fy_token_out");
        // println!("a {:?}", a);

        // za = base_reserves ** a
        let za: N =
            pow(base_reserves, a).map_err(|_| YieldMathError::BaseInForFyTokenOutPowOverflow)?;
        // println!("za {:?}", za);

        // ya = fy_token_reserves ** a
        let ya = pow(fy_token_reserves, a)
            .map_err(|_| YieldMathError::BaseInForFyTokenOutPowOverflow)?;
        // println!("ya {:?}", ya);

        // yx = fy_token_reserves - fy_token_amount
        let yx = fy_token_reserves
            .checked_sub(fy_token_amount)
            .ok_or(YieldMathError::TooMuchFyTokenOut)?;
        // println!("yx {:?}", yx);

        // yxa = yx ** a
        let yxa = pow(yx, a).map_err(|_| YieldMathError::BaseInForFyTokenOutPowOverflow)?;
        // println!("yxa {:?}", yxa);

        // sum = za + ya - yxa
        let sum = za
            .checked_add(ya)
            .ok_or(YieldMathError::BaseInForFyTokenOutOverflow)?
            .checked_sub(yxa)
            .ok_or(YieldMathError::BaseInForFyTokenOutOverflow)?; // z < MAX, y < MAX, a < 1. It can only underflow, not overflow.

        if sum < N::from_num(0) {
            return Err(YieldMathError::FyTokenReservesTooLow);
        }

        // println!("sum {:?}", sum);
        let one = N::from_num(1);
        let inv_a = one
            .checked_div(a)
            .ok_or(YieldMathError::FyTokenInForBaseOutOverflow)?; // z < MAX, y < MAX, a < 1. It can only underflow, not overflow.

        // println!("inv_a {:?}", inv_a);

        // result = (sum ** (1/a)) - base_reserves
        let sum_pow: N =
            pow(sum, inv_a).map_err(|_| YieldMathError::FyTokenInForBaseOutPowOverflow)?;

        let result = sum_pow.checked_sub(base_reserves);

        // println!(
        //     "sum ** 1/a {:?} base_reserves {:?}",
        //     sum.pow(inv_a).unwrap(),
        //     base_reserves
        // );

        // println!("");

        result.ok_or(YieldMathError::TooMuchBaseIn)
    }

    fn max_fy_token_out(
        base_reserves: N,
        fy_token_reserves: N,
        time_till_maturity: N,
        ts: N,
        g: N,
    ) -> Result<N, YieldMathError> {
        let a = Self::compute_a(time_till_maturity, ts, g, false)?;

        let xa: N = pow(base_reserves, a).map_err(|_| YieldMathError::MaxFyTokenOutPowOverflow)?;

        let ya = pow(fy_token_reserves, a).map_err(|_| YieldMathError::MaxFyTokenOutPowOverflow)?;

        let two = Self::get_number(2);

        let xy2 = xa
            .checked_add(ya)
            .map(|s| {
                s.checked_div(two)
                    .ok_or(YieldMathError::MaxFyTokenOutOverflow)
            })
            .ok_or(YieldMathError::MaxFyTokenOutOverflow)??;

        let one = N::from_num(1);
        let inv_a = one
            .checked_div(a)
            .ok_or(YieldMathError::MaxFyTokenOutOverflow)?;

        let inaccessible: N =
            pow(xy2, inv_a).map_err(|_| YieldMathError::MaxFyTokenOutPowOverflow)?;

        let zero = N::from_num(0);

        let result = if inaccessible.gt(&fy_token_reserves) {
            zero
        } else {
            fy_token_reserves.sub(inaccessible)
        };

        Ok(result)
    }

    fn max_fy_token_in(
        base_reserves: N,
        fy_token_reserves: N,
        time_till_maturity: N,
        ts: N,
        g: N,
    ) -> Result<N, YieldMathError> {
        let b = Self::compute_b(time_till_maturity, ts, g)?;

        // xa = base_reserves ** a
        // let xa = base_reserves
        //     .pow(b)
        //     .map_err(|_| YieldMathError::MaxFyTokenInPowOverflow)?;

        let xa: N = pow(base_reserves, b).map_err(|_| YieldMathError::MaxFyTokenInPowOverflow)?;

        // ya = fy_token_reserves ** a
        // let ya = fy_token_reserves
        //     .pow(b)
        //     .map_err(|_| YieldMathError::MaxFyTokenInPowOverflow)?;

        let ya: N =
            pow(fy_token_reserves, b).map_err(|_| YieldMathError::MaxFyTokenInPowOverflow)?;

        let one = N::from_num(1);
        let inv_b = one
            .checked_div(b)
            .ok_or(YieldMathError::MaxFyTokenInOverflow)?;

        let result = xa
            .checked_add(ya)
            .map(|s| pow(s, inv_b))
            .ok_or(YieldMathError::MaxFyTokenInOverflow)?
            .map_err(|_| YieldMathError::MaxFyTokenInPowOverflow)
            .map(|p: N| p.checked_sub(fy_token_reserves))?
            .ok_or(YieldMathError::MaxFyTokenInOverflow);

        result
    }

    fn max_base_in(
        base_reserves: N,
        fy_token_reserves: N,
        time_till_maturity: N,
        ts: N,
        g: N,
    ) -> Result<N, Self::Error> {
        let _max_fy_token_out =
            Self::max_fy_token_out(base_reserves, fy_token_reserves, time_till_maturity, ts, g)?;
        let zero = N::from_num(0);
        let result = if _max_fy_token_out.gt(&zero) {
            Self::base_in_for_fy_token_out(
                base_reserves,
                fy_token_reserves,
                _max_fy_token_out,
                time_till_maturity,
                ts,
                g,
            )
        } else {
            Ok(zero)
        };
        result
    }

    fn max_base_out(
        base_reserves: N,
        fy_token_reserves: N,
        time_till_maturity: N,
        ts: N,
        g: N,
    ) -> Result<N, Self::Error> {
        let _max_fy_token_in =
            Self::max_fy_token_in(base_reserves, fy_token_reserves, time_till_maturity, ts, g)?;

        Self::base_out_for_fy_token_in(
            base_reserves,
            fy_token_reserves,
            _max_fy_token_in,
            time_till_maturity,
            ts,
            g,
            false,
        )
    }

    fn compute_a(
        time_till_maturity: N,
        ts: N,
        g: N,
        calc_price: bool,
    ) -> Result<N, YieldMathError> {
        let t = ts
            .checked_mul(time_till_maturity)
            .ok_or(YieldMathError::ComputeAOverflow)?;
        let zero = N::from_num(0);

        if t.lt(&zero) {
            return Err(YieldMathError::NegativeTime);
        }

        let one = N::from_num(1);

        // a = (1 - gt)
        let a = one
            .checked_sub(g.checked_mul(t).ok_or(YieldMathError::ComputeAOverflow)?)
            .ok_or(YieldMathError::ComputeAOverflow)?;
        if !calc_price && (a.lt(&zero) || a == zero) {
            return Err(YieldMathError::TooFarFromMaturity);
        }

        if a.gt(&one) {
            return Err(YieldMathError::CoeffGMustBePositive);
        }

        Ok(a)
    }
    fn compute_b(time_till_maturity: N, ts: N, g: N) -> Result<N, YieldMathError> {
        let t = ts
            .checked_mul(time_till_maturity)
            .ok_or(YieldMathError::ComputeBOverflow)?;

        let zero = N::from_num(0);

        if t.lt(&zero) {
            return Err(YieldMathError::NegativeTime);
        }

        let one = N::from_num(1);

        // b = (1 - t/g)
        let b = one
            .checked_sub(t.checked_div(g).ok_or(YieldMathError::NegativeTime)?)
            .ok_or(YieldMathError::NegativeTime)?;

        if b.lt(&zero) || b == zero {
            return Err(YieldMathError::TooFarFromMaturity);
        }

        if b.gt(&one) {
            return Err(YieldMathError::CoeffGMustBePositive);
        }

        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use frame_support::assert_ok;
    use substrate_fixed::types::I64F64;

    use super::*;

    #[test]
    fn invariant_succesfull() {
        let base_reserves = I64F64::from_num(99_000_000);
        let fy_token_reserves = I64F64::from_num(90_000_000);
        let total_supply = I64F64::from_num(80_000_000);
        let days_from_start = 1;
        let time_till_maturity = I64F64::from_num(60 * 60 * 24 * (672 - days_from_start));
        let ts = I64F64::from_num(1) / I64F64::from_num(60 * 60 * 24 * 365 * 2);
        let g2 = I64F64::from_num(1000) / I64F64::from_num(950);

        let invariant = YieldMath::<I64F64, YieldConvert>::invariant(
            base_reserves,
            fy_token_reserves,
            total_supply,
            time_till_maturity,
            ts,
            g2,
            false,
        );

        assert_ok!(invariant);
    }

    #[test]
    fn methods_successfull() {
        let base_balances: Vec<I64F64> = vec![
            I64F64::from_num(100_000),
            I64F64::from_num(1_000_000),
            I64F64::from_num(10_000_000),
            I64F64::from_num(100_000_000),
        ];
        let fy_token_balance_deltas: Vec<I64F64> = vec![
            I64F64::from_num(0),
            I64F64::from_num(50),
            I64F64::from_num(1_000),
            I64F64::from_num(10_000),
            I64F64::from_num(100_000),
            I64F64::from_num(1_000_000),
            I64F64::from_num(10_000_000),
            I64F64::from_num(100_000_000),
        ];
        let trade_sizes: Vec<I64F64> = vec![
            I64F64::from_num(1),
            I64F64::from_num(1_000),
            I64F64::from_num(10_000),
            I64F64::from_num(100_000),
        ];
        let times_till_maturity: Vec<I64F64> = vec![
            I64F64::from_num(7 * 24 * 60 * 60),
            I64F64::from_num(0),
            I64F64::from_num(1),
            I64F64::from_num(4),
            I64F64::from_num(40),
            I64F64::from_num(400),
            I64F64::from_num(4_000),
            I64F64::from_num(400_000),
            I64F64::from_num(40_000_000),
        ];

        let seconds_in_one_year = I64F64::from_num(31_557_600);
        let seconds_in_ten_years = seconds_in_one_year
            .checked_mul(I64F64::from_num(10))
            .unwrap();
        let ts = I64F64::from_num(1)
            .checked_div(seconds_in_ten_years)
            .unwrap();
        let g1 = I64F64::from_num(950)
            .checked_div(I64F64::from_num(1000))
            .unwrap(); // Sell base to the pool
        let g2 = I64F64::from_num(950)
            .checked_div(I64F64::from_num(950))
            .unwrap(); // Sell fyToken to the pool

        for base_balance in base_balances {
            for fy_token_balance_delta in &fy_token_balance_deltas {
                for trade_size in &trade_sizes {
                    for time_till_maturity in &times_till_maturity {
                        // println!("\n------------------\nbase_balance, fy_token_balance_delta, trade_size, time_till_maturity\n{:?} {:?} {:?} {:?}", base_balance, fy_token_balance_delta, trade_size, time_till_maturity);
                        let fy_token_balance = base_balance
                            .checked_add(*fy_token_balance_delta)
                            .expect("No overflow");
                        let base_out_for_fy_token_in =
                            YieldMath::<I64F64, YieldConvert>::base_out_for_fy_token_in(
                                base_balance,
                                fy_token_balance,
                                *trade_size,
                                *time_till_maturity,
                                ts,
                                g2,
                                false,
                            );
                        assert_ok!(base_out_for_fy_token_in.clone());
                        // println!("base_out_for_fy_token_in {:?}\n", base_out_for_fy_token_in);

                        let fy_token_out_for_base_in =
                            YieldMath::<I64F64, YieldConvert>::fy_token_out_for_base_in(
                                base_balance,
                                fy_token_balance,
                                *trade_size,
                                *time_till_maturity,
                                ts,
                                g1,
                            );

                        assert_ok!(fy_token_out_for_base_in.clone());
                        // println!("fy_token_out_for_base_in {:?}\n", fy_token_out_for_base_in);

                        let fy_token_in_for_base_out =
                            YieldMath::<I64F64, YieldConvert>::fy_token_in_for_base_out(
                                base_balance,
                                fy_token_balance,
                                *trade_size,
                                *time_till_maturity,
                                ts,
                                g2,
                            );
                        assert_ok!(fy_token_in_for_base_out.clone());
                        // println!("fy_token_in_for_base_out {:?}\n", fy_token_in_for_base_out);

                        let base_in_for_fy_token_out =
                            YieldMath::<I64F64, YieldConvert>::base_in_for_fy_token_out(
                                base_balance,
                                fy_token_balance,
                                *trade_size,
                                *time_till_maturity,
                                ts,
                                g1,
                            );
                        assert_ok!(base_in_for_fy_token_out.clone());
                        // println!("base_in_for_fy_token_out {:?}\n", base_in_for_fy_token_out);
                    }
                }
            }
        }
    }

    fn values() -> [[I64F64; 4]; 3] {
        [
            [
                I64F64::from_num(10_000),
                I64F64::from_num(1_000),
                I64F64::from_num(10),
                I64F64::from_num(1_000_000),
            ],
            [
                I64F64::from_num(100_000),
                I64F64::from_num(10_000),
                I64F64::from_num(100),
                I64F64::from_num(1_000_000),
            ],
            [
                I64F64::from_num(100_000_000),
                I64F64::from_num(10_000_000),
                I64F64::from_num(1_000),
                I64F64::from_num(1_000_000),
            ],
        ]
    }

    const TIME_TILL_MATURITY: [u64; 5] = [0, 40, 4000, 400000, 40000000];

    fn ts_sub_fixed() -> I64F64 {
        let seconds_in_one_year = I64F64::from_num(31_557_600);
        let seconds_in_ten_years = seconds_in_one_year
            .checked_mul(I64F64::from_num(10))
            .unwrap();
        let ts = I64F64::from_num(1)
            .checked_div(seconds_in_ten_years)
            .unwrap();
        ts
    }

    #[test]
    fn fy_token_out_for_base_in_return_more_with_higher_g() {
        let ts = ts_sub_fixed();
        let mut previous_result = I64F64::from_num(0);
        for v in values() {
            let base_balance = v[0];
            let fy_token_balance = v[1];
            let base_amount = v[2];
            let time_till_maturity = v[3];

            let g = [
                I64F64::from_num(0.9),
                I64F64::from_num(0.95),
                I64F64::from_num(0.95),
            ];
            let mut result = I64F64::from_num(0);
            for g_ in g {
                result = YieldMath::<I64F64, YieldConvert>::fy_token_out_for_base_in(
                    base_balance,
                    fy_token_balance,
                    base_amount,
                    time_till_maturity,
                    ts,
                    g_,
                )
                .unwrap();
            }

            assert!(result.gt(&previous_result));
            previous_result = result;
        }
    }

    #[test]
    fn fy_token_out_for_base_in_price_growth_to_one_as_we_approach_maturity() {
        let ts = ts_sub_fixed();
        let g1 = I64F64::from_num(0.95);
        for v in values() {
            let base_balance = v[0];
            let fy_token_balance = v[1];
            let base_amount = v[2];

            let maximum = base_amount;
            let mut previous_result = maximum;
            for time_till_maturity in TIME_TILL_MATURITY {
                let result = YieldMath::<I64F64, YieldConvert>::fy_token_out_for_base_in(
                    base_balance,
                    fy_token_balance,
                    base_amount,
                    I64F64::from_num(time_till_maturity),
                    ts,
                    g1,
                )
                .expect("No overflow");

                // println!("      fy_token_out_for_base_in {:?}", result);
                // println!("      time_till_maturity {:?}", time_till_maturity);
                if time_till_maturity == 0 {
                    // Test that when we are very close to maturity, price is very close to 1
                    assert_eq!(result, maximum);
                } else {
                    // Easier to test prices diverging from 1
                    assert!(result.lt(&previous_result));
                }
                previous_result = result
            }
        }
    }

    #[test]
    fn base_out_for_fy_token_in_lower_g_means_more_base_out() {
        let ts = ts_sub_fixed();
        let mut previous_result = I64F64::from_num(0);
        for value in values() {
            let base_balance = value[0];
            let fy_token_balance = value[1];
            let base_amount = value[2];
            let time_till_maturity = value[3];

            let g = [
                I64F64::from_num(0.95),
                I64F64::from_num(0.95),
                I64F64::from_num(0.9),
            ];
            let mut result = I64F64::from_num(0);
            for g_value in g {
                result = YieldMath::<I64F64, YieldConvert>::base_out_for_fy_token_in(
                    base_balance,
                    fy_token_balance,
                    base_amount,
                    time_till_maturity,
                    ts,
                    g_value,
                    false,
                )
                .expect("No overflow");

                // println!("base_out_for_fy_token_in {:?}", result);
                // println!("base_balance {:?}", base_balance);
                // println!("fy_token_balance {:?}", fy_token_balance);
                // println!("base_amount {:?}", base_amount);
                // println!("time_till_maturity {:?}", time_till_maturity);
                // println!("g_value {:?}\n", g_value);
            }

            assert!(result.gt(&previous_result));
            previous_result = result
        }
    }

    #[test]
    fn base_out_for_fy_token_in_on_maturity_price_drops_to_1() {
        let ts = ts_sub_fixed();
        let g2 = I64F64::from_num(1000) / I64F64::from_num(950);

        for value in values() {
            let base_balance = value[0];
            let fy_token_balance = value[1];
            let fy_token_amount = value[2];

            let minimum = fy_token_amount;
            let mut previous_result = minimum;
            for time_till_maturity in TIME_TILL_MATURITY {
                let result = YieldMath::<I64F64, YieldConvert>::base_out_for_fy_token_in(
                    base_balance,
                    fy_token_balance,
                    fy_token_amount,
                    I64F64::from_num(time_till_maturity),
                    ts,
                    g2,
                    false,
                )
                .expect("No overflow");

                println!("base_balance {:?}", base_balance);
                println!("fy_token_balance {:?}", fy_token_balance);
                println!("fy_token_amount {:?}", fy_token_amount);
                println!("time_till_maturity {:?}", time_till_maturity);
                println!("base_out_for_fy_token_in {:?}", result);
                println!("previous_result {:?}", previous_result);
                println!("");
                if time_till_maturity == 0 {
                    // Test that when we are very close to maturity, price is very close to 1 minus flat fee.
                    assert_eq!(result, minimum);
                } else {
                    // Easier to test prices diverging from 1

                    assert!(result.gt(&previous_result));
                }
                previous_result = result
            }
        }
    }

    #[test]
    fn fy_token_in_for_base_out_higher_g_means_more_fy_token() {
        let ts = ts_sub_fixed();
        let mut previous_result = I64F64::from_num(0);
        for value in values() {
            let base_balance = value[0];
            let fy_token_balance = value[1];
            let base_amount = value[2];
            let time_till_maturity = value[3];

            let g = [
                I64F64::from_num(0.9),
                I64F64::from_num(0.95),
                I64F64::from_num(0.95),
            ];
            let mut result = I64F64::from_num(0);
            for g_value in g {
                result = YieldMath::<I64F64, YieldConvert>::fy_token_in_for_base_out(
                    base_balance,
                    fy_token_balance,
                    base_amount,
                    time_till_maturity,
                    ts,
                    g_value,
                )
                .expect("No overflow");
            }

            assert!(result > previous_result);
            previous_result = result
        }
    }

    //   it("As we approach maturity, price grows to 1 for `fy_token_in_for_base_out`", async () => {
    #[test]
    fn fy_token_in_for_base_out_on_maturity_price_rise_to_1() {
        let ts = ts_sub_fixed();
        let g2 = I64F64::from_num(1000) / I64F64::from_num(950);
        //     for (var i = 0; i < values.length; i++) {
        for value in values() {
            let base_balance = value[0];
            let fy_token_balance = value[1];
            let base_amount = value[2];

            let mut previous_result = base_amount;
            //       for (var j = 0; j < time_till_maturity.length; j++) {
            for time_till_maturity in TIME_TILL_MATURITY {
                //         var t = time_till_maturity[j]
                //         result = await yieldMath.fy_token_in_for_base_out(base_balance, fy_token_balance, base_amount, t, ts, g2)
                let result = YieldMath::<I64F64, YieldConvert>::fy_token_in_for_base_out(
                    base_balance,
                    fy_token_balance,
                    base_amount,
                    I64F64::from_num(time_till_maturity),
                    ts,
                    g2,
                )
                .expect("No overflow");

                //         if (j == 0) {
                if time_till_maturity == 0 {
                    //           // Test that when we are on maturity, price is equal 1
                    assert_eq!(result, base_amount);
                //           almostEqual(result, maximum, PRECISION)
                } else {
                    //           // Easier to test prices diverging from 1
                    //           expect(result).to.be.lt(previous_result)
                    assert!(result < previous_result)
                }
                previous_result = result
            }
        }
        //   })
    }

    #[test]
    fn base_in_for_fy_token_out_lower_g_means_more_base_in() {
        let ts = ts_sub_fixed();
        let mut previous_result = I64F64::from_num(0);
        for value in values() {
            let base_balance = value[0];
            let fy_token_balance = value[1];
            let base_amount = value[2];
            let time_till_maturity = value[3];

            let g = [
                I64F64::from_num(950) / I64F64::from_num(1000),
                I64F64::from_num(95) / I64F64::from_num(100),
                I64F64::from_num(9) / I64F64::from_num(10),
            ];
            let mut result = I64F64::from_num(0);
            for g_value in g {
                result = YieldMath::<I64F64, YieldConvert>::base_in_for_fy_token_out(
                    base_balance,
                    fy_token_balance,
                    base_amount,
                    time_till_maturity,
                    ts,
                    g_value,
                )
                .expect("No overflow");
            }

            assert!(result > previous_result);
            previous_result = result
        }
    }

    //   it("As we approach maturity, price drops to 1 for `base_in_for_fy_token_out`", async () => {
    #[test]
    fn base_in_for_fy_token_out_on_maturity_price_drops_to_1() {
        let ts = ts_sub_fixed();
        let g1 = I64F64::from_num(0.95);
        for value in values() {
            let base_balance = value[0];
            let fy_token_balance = value[1];
            let base_amount = value[2];

            let mut result = base_amount;
            let mut previous_result = result;
            for time_till_maturity in TIME_TILL_MATURITY {
                result = YieldMath::<I64F64, YieldConvert>::base_in_for_fy_token_out(
                    base_balance,
                    fy_token_balance,
                    base_amount,
                    I64F64::from_num(time_till_maturity),
                    ts,
                    g1,
                )
                .expect("No overflow");

                if time_till_maturity == 0 {
                    // Test that when we are on maturity, price is equal to 1
                    assert_eq!(result, base_amount);
                } else {
                    // Easier to test prices diverging from 1
                    assert!(result > previous_result);
                }
                previous_result = result
            }
        }
    }
}
