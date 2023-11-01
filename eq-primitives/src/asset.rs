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

use crate::balance_number::EqFixedU128;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::Parameter;
use impl_trait_for_tuples::impl_for_tuples;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{FixedI64, FixedPointNumber, Percent, Permill};
use sp_std::{cmp::Ordering, convert::TryInto, fmt::Debug, str::FromStr, vec::Vec};
use xcm::v3::{AssetId, Junction::*, Junctions::*, MultiLocation};

extern crate alloc;
use alloc::string::String;

pub type AssetIdInnerType = u64;

#[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
/// Struct with asset params
pub struct AssetData<Asset> {
    pub id: Asset, //str -> [u8; 8] -> u64
    pub lot: EqFixedU128,
    pub price_step: FixedI64,
    pub maker_fee: Permill,
    pub taker_fee: Permill,
    pub asset_xcm_data: AssetXcmData,
    pub debt_weight: Permill,
    pub lending_debt_weight: Permill,
    pub buyout_priority: u64,
    pub asset_type: AssetType,
    pub is_dex_enabled: bool, // can be used in dex
    pub collateral_discount: Percent,
}

impl AssetData<Asset> {
    pub fn gen_multi_location(&self) -> (MultiLocation, u8) {
        let id: Vec<u8> = self.id.to_str_bytes();
        let mut data = [0u8; 32];
        data[..id.len()].copy_from_slice(&id[..]);
        let multi_location = MultiLocation {
            parents: 0,
            interior: match self.asset_type {
                AssetType::Native => Here,
                _ => X1(GeneralKey {
                    length: id.len() as u8,
                    data,
                }),
            },
        };
        (multi_location, crate::DECIMALS)
    }

    pub fn get_xcm_data(&self) -> Option<(MultiLocation, u8, bool)> {
        match &self.asset_xcm_data {
            AssetXcmData::None => None,
            AssetXcmData::SelfReserved => {
                let (multi_location, decimals) = self.gen_multi_location();
                Some((multi_location, decimals, true))
            }
            AssetXcmData::OtherReserved(OtherReservedData {
                multi_location,
                decimals,
            }) => Some((multi_location.clone(), *decimals, false)),
        }
    }
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Debug, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AmmPool {
    Curve(equilibrium_curve_amm::PoolId),
    Yield(crate::xdot_pool::PoolId),
}

#[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
pub struct OtherReservedData {
    pub multi_location: MultiLocation,
    pub decimals: u8,
}

#[derive(Decode, Encode, Clone, Debug, Eq, PartialEq, scale_info::TypeInfo)]
pub enum AssetXcmData {
    /// Token cannot be transfered via XCM
    None,
    /// Token that belong to this parachain
    SelfReserved,
    /// Token from another parachain
    OtherReserved(OtherReservedData),
}

impl Default for AssetXcmData {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Debug, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AssetType {
    /// Main asset
    Native,
    /// Regular token type, examples: DOT, ETH, BTC
    Physical,
    /// Synthetic token, example: EQD
    Synthetic,
    /// Liquidity pool token
    Lp(AmmPool),
}

// well known assets
pub const EQD: Asset = Asset(6648164); //::from_bytes(b"eqd"); 0x657164
pub const BTC: Asset = Asset(6452323); //::from_bytes(b"btc"); 0x627463
pub const ETH: Asset = Asset(6648936); //::from_bytes(b"eth"); 0x657468
pub const CRV: Asset = Asset(6517366); //::from_bytes(b"crv"); 0x637276
pub const EOS: Asset = Asset(6647667); //::from_bytes(b"eos"); 0x656F73
pub const EQ: Asset = Asset(25969); //::from_bytes(b"eq"); 0x6571
pub const Q: Asset = Asset(113); //::from_bytes(b"q"); 0x71
pub const GENS: Asset = Asset(1734700659); //::from_bytes(b"gens"); 0x67656E73
pub const DAI: Asset = Asset(6578537); //::from_bytes(b"dai"); 0x646169
pub const USDT: Asset = Asset(1970496628); //::from_bytes(b"usdt"); 0x75736474
pub const BUSD: Asset = Asset(1651864420); //::from_bytes(b"busd"); 0x62757364
pub const USDC: Asset = Asset(1970496611); //::from_bytes(b"usdc"); 0x75736463
pub const BNB: Asset = Asset(6450786); //::from_bytes(b"bnb"); 0x626E62
pub const WBTC: Asset = Asset(2002941027); //::from_bytes(b"wbtc"); 0x77627463
pub const HDOT: Asset = Asset(1751412596); //::from_bytes(b"hdot"); 0x68646f74
pub const XDOT: Asset = Asset(2019848052); //::from_bytes(b"xdot"); 0x78646F74
pub const XDOT2: Asset = Asset(517081101362); //::from_bytes(b"xdot2");0x78646F7432
pub const XDOT3: Asset = Asset(517081101363); //::from_bytes(b"xdot3"); 0x78646F7433
pub const YDOT: Asset = Asset(2036625268); //::from_bytes(b"ydot"); 0x79646F74
pub const EQDOT: Asset = Asset(435694104436); //::from_bytes(b"eqdot"); 0x6571646F74
pub const MXETH: Asset = Asset(470171350120); //::from_bytes(b"mxeth"); 0x6d78657468
pub const MXWBTC: Asset = Asset(120364166444131); //::from_bytes(b"mxwbtc"); 0x6d7877627463
pub const MXUSDC: Asset = Asset(120364133999715); //::from_bytes(b"mxusdc"); 0x6d7875736463
pub const LIT: Asset = Asset(7104884);
pub const PDEX: Asset = Asset(1885627768);

// Polkadot relay-chain
pub const DOT: Asset = Asset(6582132); //::from_bytes(b"dot"); 0x646F74
pub const GLMR: Asset = Asset(1735159154); //::from_bytes(b"glmr"); 0x676C6D72
pub const MATIC: Asset = Asset(469786454371); //::from_bytes(b"matic"); 0x6D61746963
pub const XCBNB: Asset = Asset(517063470690); //::from_bytes(b"xcbnb"); 0x7863626E62
pub const PARA: Asset = Asset(1885434465); //::from_bytes(b"para"); 0x70617261
pub const ACA: Asset = Asset(6382433); //::from_bytes(b"aca"); 0x616361
pub const AUSD: Asset = Asset(1635087204); //::from_bytes(b"ausd"); 0x61757364
pub const IBTC: Asset = Asset(1768060003); //::from_bytes(b"ibtc"); 0x69627463
pub const ASTR: Asset = Asset(1634956402); //::from_bytes(b"astr");
pub const INTR: Asset = Asset(1768846450); //::from_bytes(b"intr");
pub const STDOT: Asset = Asset(495873978228); // ::from_bytes(b"stdot")
pub const LDO: Asset = Asset(7103599); //::from_bytes(b"ldo")
pub const FRAX: Asset = Asset(1718772088); //::from_bytes(b"frax")
pub const XOR: Asset = Asset(7892850); //::from_bytes(b"xor")
pub const LPDOT: Asset = Asset(465742098292); //::from_bytes(b"lpdot")
pub const CDOT613: Asset = Asset(426883035443); //::from_bytes(b"cd613"); 0x6364363133
pub const CDOT714: Asset = Asset(426883100980); //::from_bytes(b"cd714"); 0x6364373134
pub const CDOT815: Asset = Asset(426883166517); //::from_bytes(b"cd815"); 0x6364383135
pub const CRU: Asset = Asset(6517365); //::from_bytes(b"cru"); 0x637275
pub const PHA: Asset = Asset(7366753); //::from_bytes(b"pha"); 0x706861
pub const VDOT: Asset = Asset(1986293620); //::from_bytes(b"vdot"); 0x76646F74
pub const LDOT: Asset = Asset(1818521460); //::from_bytes(b"ldot"); 0x6C646F74
pub const SDOT: Asset = Asset(1935961972); //::from_bytes(b"sdot"); 0x73646F74
pub const TDOT: Asset = Asset(1952739188); //::from_bytes(b"tdot"); 0x74646f74

// Kusama relay-chain
pub const KSM: Asset = Asset(7041901); //::from_bytes(b"ksm"); 0x6B736D
pub const MOVR: Asset = Asset(1836021362); //::from_bytes(b"movr"); 0x6D6F7672
pub const HKO: Asset = Asset(6843247); //::from_bytes(b"hko"); 0x686B6F
pub const KAR: Asset = Asset(7037298); //::from_bytes(b"kar"); 0x6B6172
pub const KUSD: Asset = Asset(1802859364); //::from_bytes(b"kusd""); 0x6B757364
pub const LKSM: Asset = Asset(1818981229); //::from_bytes(b"lksm"); 0x6C6B736D
pub const KBTC: Asset = Asset(1801614435); //::from_bytes(b"kbtc"); 0x6B627463
pub const SDN: Asset = Asset(7562350); //::from_bytes(b"sdn"); 0x73646E
pub const BNC: Asset = Asset(6450787); //::from_bytes(b"bnc"); 0x626E63

impl<Asset: Parameter + Ord + Copy> AssetData<Asset> {
    pub fn new(
        id: Asset,
        lot: EqFixedU128,
        price_step: FixedI64,
        maker_fee: Permill,
        taker_fee: Permill,
        asset_xcm_data: AssetXcmData,
        debt_weight: Permill,
        buyout_priority: u64,
        asset_type: AssetType,
        is_dex_enabled: bool,
        collateral_discount: Percent,
        lending_debt_weight: Permill,
    ) -> Result<Self, AssetError> {
        if price_step.is_negative() {
            return Err(AssetError::PriceStepNegative);
        }

        Ok(AssetData {
            id,
            lot,
            price_step,
            maker_fee,
            taker_fee,
            asset_xcm_data,
            debt_weight,
            buyout_priority,
            asset_type,
            is_dex_enabled,
            collateral_discount,
            lending_debt_weight,
        })
    }
}

#[derive(
    Copy, Clone, Eq, PartialEq, Decode, Encode, Hash, Default, MaxEncodedLen, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Asset(pub AssetIdInnerType);

impl Asset {
    const SIZE_OF_ASSET_ID_INNER: usize = sp_std::mem::size_of::<AssetIdInnerType>();
    /// Creates new `Asset` instance from str
    pub fn from_bytes(asset: &[u8]) -> Result<Self, AssetError> {
        if Self::wrong_len(asset) {
            return Err(AssetError::AssetNameWrongLength);
        }
        // only latin and 0-9 is allowed
        // convert uppercase to lowercase
        let asset = Self::lower_case(asset).ok_or(AssetError::AssetNameWrongSymbols)?;

        let saturating_zeros = [0_u8; Self::SIZE_OF_ASSET_ID_INNER];

        let saturated = [&saturating_zeros, asset.as_slice()].concat();
        let saturated = &saturated[asset.len()..];
        let saturated_arr = Self::slice_to_arr(&saturated);
        let id = AssetIdInnerType::from_be_bytes(*saturated_arr);
        Ok(Self(id))
    }

    fn slice_to_arr(s: &[u8]) -> &[u8; Self::SIZE_OF_ASSET_ID_INNER] {
        s.try_into().expect("slice with incorrect length")
    }

    fn wrong_len(asset: &[u8]) -> bool {
        asset.len() > Self::SIZE_OF_ASSET_ID_INNER
    }

    fn lower_case(asset: &[u8]) -> Option<Vec<u8>> {
        let digits = 48_u8..58_u8;
        let latin_lower_case = 97_u8..123_u8;
        let latin_upper_case = 65_u8..91_u8;

        let asset_lower_case = asset.iter().map(|b| {
            if latin_upper_case.contains(b) {
                b + 32_u8
            } else {
                *b
            }
        });

        let wrong_symbols = asset_lower_case
            .clone()
            .any(|b| !(digits.contains(&b) || latin_lower_case.contains(&b)));
        if wrong_symbols {
            None
        } else {
            Some(asset_lower_case.collect())
        }
    }

    /// Returns original bytes which can be used in from_utf8 to get str
    pub fn to_str_bytes(&self) -> Vec<u8> {
        let bytes = self.0.to_be_bytes();
        let bytes: Vec<u8> = bytes.iter().cloned().filter(|b| b != &0_u8).collect();
        bytes
    }

    pub fn to_str(&self) -> String {
        String::from_utf8(self.to_str_bytes()).expect("Asset contains valid utf8 str")
    }

    pub fn get_id(&self) -> AssetIdInnerType {
        self.0
    }

    pub fn new(id: AssetIdInnerType) -> Result<Self, AssetError> {
        Self::from_bytes(&Self(id).to_str_bytes())
    }
}

impl FromStr for Asset {
    type Err = AssetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_bytes(s.as_bytes())
    }
}

#[cfg(feature = "std")]
impl ToString for Asset {
    fn to_string(&self) -> String {
        self.to_str()
    }
}

impl sp_std::fmt::Debug for Asset {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        f.write_fmt(format_args!("${}", self.to_str().to_uppercase()))
    }
}

impl sp_std::cmp::PartialOrd for Asset {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }

    fn lt(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Less))
    }

    fn le(&self, other: &Self) -> bool {
        // Pattern `Some(Less | Eq)` optimizes worse than negating `None | Some(Greater)`.
        // FIXME: The root cause was fixed upstream in LLVM with:
        // https://github.com/llvm/llvm-project/commit/9bad7de9a3fb844f1ca2965f35d0c2a3d1e11775
        // Revert this workaround once support for LLVM 12 gets dropped.
        !matches!(self.partial_cmp(other), None | Some(Ordering::Greater))
    }

    fn gt(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Greater))
    }

    fn ge(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Equal))
            || matches!(self.partial_cmp(other), Some(Ordering::Greater))
    }
}

impl sp_std::cmp::Ord for Asset {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }

    fn max(self, other: Self) -> Self
    where
        Self: Sized,
    {
        let (Self(f), Self(s)) = (self, other);
        if f.max(s) == f {
            self
        } else {
            other
        }
    }

    fn min(self, other: Self) -> Self
    where
        Self: Sized,
    {
        let (Self(f), Self(s)) = (self, other);
        if f.min(s) == f {
            self
        } else {
            other
        }
    }

    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized,
    {
        Self(self.0.clamp(min.0, max.0))
    }
}

/// Assets reading interface
pub trait AssetGetter {
    fn get_asset_data(asset: &Asset) -> Result<AssetData<Asset>, sp_runtime::DispatchError>;

    fn exists(asset: Asset) -> bool;

    fn get_assets_data() -> Vec<AssetData<Asset>>;

    fn get_assets_data_with_usd() -> Vec<AssetData<Asset>>;

    fn get_assets() -> Vec<Asset>;

    fn get_assets_with_usd() -> Vec<Asset>;

    fn priority(asset: Asset) -> Option<u64>;

    fn get_main_asset() -> Asset;

    fn collateral_discount(asset: &Asset) -> EqFixedU128;
}

pub trait AssetXcmGetter {
    fn get_self_reserved_xcm_assets() -> Vec<AssetId>;

    fn get_other_reserved_xcm_assets() -> Vec<AssetId>;
}

#[derive(Debug)]
pub enum AssetError {
    DebtWeightNegative,
    DebtWeightMoreThanOne,
    AssetNameWrongLength,
    AssetNameWrongSymbols,
    PriceStepNegative,
    CollateralDiscountNegative,
}

#[impl_for_tuples(5)]
pub trait OnNewAsset {
    fn on_new_asset(asset: Asset, prices: Vec<sp_runtime::FixedI64>);
}

#[macro_export]
macro_rules! str_asset {
    ($asset:expr) => {
        core::str::from_utf8(&$asset.to_str_bytes());
    };
}
