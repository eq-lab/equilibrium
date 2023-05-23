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
//! # Equilibrium Claim Pallet
//!
//! Equilibrium's Balances Pallet is a Substrate module that processes claims
//! from Ethereum addresses. It is used in Equilibrium Substrate to payout
//! claims generated during Token Swap event

mod benchmarking;
mod mock;
mod secp_utils;
pub mod weights;

use codec::{Decode, Encode};
use core::convert::TryInto;
use eq_utils::{eq_ensure, ok_or_error};
#[allow(unused_imports)]
use frame_support::debug; // This usage is required by a macro
use frame_support::{
    traits::{Currency, EnsureOrigin, Get, IsSubType, VestingSchedule},
    weights::{DispatchClass, Pays},
};
use frame_system::{ensure_none, ensure_root, ensure_signed};
#[cfg(feature = "std")]
use serde::{self, Deserialize, Deserializer, Serialize, Serializer};
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub, DispatchInfoOf, Saturating, SignedExtension},
    transaction_validity::{
        InvalidTransaction, TransactionLongevity, TransactionSource, TransactionValidity,
        TransactionValidityError, ValidTransaction,
    },
    ArithmeticError, DispatchResult, RuntimeDebug,
};
use sp_std::{fmt::Debug, prelude::*};

pub use weights::WeightInfo;

type CurrencyOf<T> = <<T as Config>::VestingSchedule as VestingSchedule<
    <T as frame_system::Config>::AccountId,
>>::Currency;
type BalanceOf<T> = <CurrencyOf<T> as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Claim validation errors
#[repr(u8)]
pub enum ValidityError {
    /// The Ethereum signature is invalid
    InvalidEthereumSignature = 0,
    /// The signer has no claim
    SignerHasNoClaim = 1,
    /// An invalid statement was made for a claim
    InvalidStatement = 2,
}

impl From<ValidityError> for u8 {
    fn from(err: ValidityError) -> Self {
        err as u8
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WeightInfo: WeightInfo;
        /// Used to schedule vesting part of a claim
        type VestingSchedule: VestingSchedule<Self::AccountId, Moment = Self::BlockNumber>;
        /// The Prefix that is used in signed Ethereum messages for this network
        type Prefix: Get<&'static [u8]>;
        /// Origin that can move claims to another account
        type MoveClaimOrigin: EnsureOrigin<Self::Origin>;
        /// Gets vesting account
        type VestingAccountId: Get<Self::AccountId>;
        /// For unsigned transaction priority calculation
        type UnsignedPriority: Get<TransactionPriority>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Make a claim to collect your currency.
        ///
        /// The dispatch origin for this call must be _None_.
        ///
        /// Unsigned Validation:
        /// A call to claim is deemed valid if the signature provided matches
        /// the expected signed message of:
        ///
        /// > Ethereum Signed Message:
        /// > (configured prefix string)(address)
        ///
        /// and `address` matches the `dest` account.
        ///
        /// Parameters:
        /// - `dest`: The destination account to payout the claim.
        /// - `ethereum_signature`: The signature of an ethereum signed message
        ///    matching the format described above.
        #[pallet::weight(T::WeightInfo::claim())]
        pub fn claim(
            origin: OriginFor<T>,
            dest: T::AccountId,
            ethereum_signature: EcdsaSignature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            let data = dest.using_encoded(to_ascii_hex);
            let option_ethereum_address = Self::eth_recover(&ethereum_signature, &data, &[][..]);
            let signer = ok_or_error!(option_ethereum_address, Error::<T>::InvalidEthereumSignature,
            "{}:{}. Invalid ethereum signature while recover. Dest: {:?}, signature: {:?}, data: {:?}.",
            file!(), line!(), dest, ethereum_signature, data)?;

            eq_ensure!(
                <Signing<T>>::get(&signer) == false,
                Error::<T>::InvalidStatement,
                target: "eq_claim",
                "{}:{}. Cannot get signer. Who: {:?}.",
                file!(),
                line!(),
                signer
            );

            Self::process_claim(signer, dest)?;
            Ok(().into())
        }

        /// Mint a new claim to collect currency
        ///
        /// The dispatch origin for this call must be _Root_.
        ///
        /// Parameters:
        /// - `who`: The Ethereum address allowed to collect this claim.
        /// - `value`: The balance that will be claimed.
        /// - `vesting_schedule`: An optional vesting schedule for the claim
        #[pallet::weight(T::WeightInfo::mint_claim())]
        pub fn mint_claim(
            origin: OriginFor<T>,
            who: EthereumAddress,
            value: BalanceOf<T>,
            vesting_schedule: Option<(BalanceOf<T>, BalanceOf<T>, T::BlockNumber)>,
            statement: bool,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            eq_ensure!(
                who != EthereumAddress::zero(),
                Error::<T>::InvalidReceiver,
                target: "eq_claim",
                "{}:{}. Minting to this address is not allowed",
                file!(),
                line!(),
            );

            if vesting_schedule != None && value < vesting_schedule.unwrap().0 {
                eq_ensure!(false, Error::<T>::InvalidStatement, target: "eq_claim",
                "{}:{}. Amount to claim is less than vesting_schedule.locked. Amount to claim: {:?}, vesting_schedule: {:?}.",
                file!(), line!(), value, vesting_schedule.unwrap().0);
            }

            let new_total_option = <Total<T>>::get().checked_add(&value);
            let new_total = ok_or_error!(
                new_total_option,
                ArithmeticError::Overflow,
                "{}:{}. Overflow mint claim, amount: {:?}",
                file!(),
                line!(),
                value
            )?;
            <Total<T>>::mutate(|t| *t = new_total);

            eq_ensure!(
                <Claims<T>>::get(&who).is_none(),
                Error::<T>::InvalidReceiver,
                target: "eq_claim",
                "{}:{}. Several claims are not allowed",
                file!(),
                line!(),
            );
            <Claims<T>>::insert(who, value);
            if let Some(vs) = vesting_schedule {
                eq_ensure!(
                    <Vesting<T>>::get(&who).is_none(),
                    Error::<T>::InvalidReceiver,
                    target: "eq_claim",
                    "{}:{}. Several vestings are not allowed",
                    file!(),
                    line!(),
                );
                <Vesting<T>>::insert(who, vs);
            }
            if statement {
                <Signing<T>>::insert(who, statement);
            }

            Ok(().into())
        }

        /// Make a claim to collect your currency by signing a statement.
        ///
        /// The dispatch origin for this call must be _None_.
        ///
        /// Unsigned Validation:
        /// A call to `claim_attest` is deemed valid if the signature provided matches
        /// the expected signed message of:
        ///
        /// > Ethereum Signed Message:
        /// > (configured prefix string)(address)(statement)
        ///
        /// and `address` matches the `dest` account; the `statement` must match that which is
        /// expected according to your purchase arrangement.
        ///
        /// Parameters:
        /// - `dest`: The destination account to payout the claim.
        /// - `ethereum_signature`: The signature of an ethereum signed message
        ///    matching the format described above.
        /// - `statement`: The identity of the statement which is being attested to in the signature.
        #[pallet::weight(T::WeightInfo::claim_attest())]
        pub fn claim_attest(
            origin: OriginFor<T>,
            dest: T::AccountId,
            ethereum_signature: EcdsaSignature,
            statement: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            let data = dest.using_encoded(to_ascii_hex);
            let option_ethereum_address = Self::eth_recover(&ethereum_signature, &data, &statement);
            let signer = ok_or_error!(option_ethereum_address, Error::<T>::InvalidEthereumSignature,
                "{}:{}. Invalid ethereum signature while recover. Dest: {:?}, signature: {:?}, data: {:?}, statement: {:?}.",
                file!(), line!(), dest, ethereum_signature, data, statement)?;
            let s = <Signing<T>>::get(signer);
            if s {
                eq_ensure!(get_statement_text() == &statement[..], Error::<T>::InvalidStatement,
                target: "eq_claim",
                "{}:{}. Get_statement_text() not equal to statement from params. Get statement text: {:?}, from params: {:?}.",
                file!(), line!(), get_statement_text(), &statement[..]);
            }
            Self::process_claim(signer, dest)?;
            Ok(().into())
        }

        /// Attest to a statement, needed to finalize the claims process.
        ///
        /// Unsigned Validation:
        /// A call to attest is deemed valid if the sender has a `Preclaim` registered
        /// and provides a `statement` which is expected for the account.
        ///
        /// Parameters:
        /// - `statement`: The identity of the statement which is being attested to in the signature.
        #[pallet::weight((T::WeightInfo::attest(), DispatchClass::Normal, Pays::No))]
        pub fn attest(origin: OriginFor<T>, statement: Vec<u8>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let option_ethereum_address = Preclaims::<T>::get(&who);
            let signer = ok_or_error!(
                option_ethereum_address,
                Error::<T>::SenderHasNoClaim,
                "{}:{}. Sender not in Preclaims. Who: {:?}.",
                file!(),
                line!(),
                who
            )?;
            let s = <Signing<T>>::get(signer);
            if s {
                eq_ensure!(get_statement_text() == &statement[..], Error::<T>::InvalidStatement,
                target: "eq_claim",
                "{}:{}. Get_statement_text() not equal to statement from params. Get statement text: {:?}, from params: {:?}.",
                file!(), line!(), get_statement_text(), &statement[..]);
            }
            Self::process_claim(signer, who.clone())?;
            Preclaims::<T>::remove(&who);
            Ok(().into())
        }

        /// Gives claims ownership from `old` to `new`
        #[pallet::weight((
            T::DbWeight::get().reads_writes(4, 4).saturating_add(Weight::from_ref_time(100_000_000_000)),
            DispatchClass::Normal,
            Pays::No
        ))]
        pub fn move_claim(
            origin: OriginFor<T>,
            old: EthereumAddress,
            new: EthereumAddress,
            maybe_preclaim: Option<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            eq_ensure!(
                old != new && new != EthereumAddress::zero(),
                Error::<T>::InvalidReceiver,
                target: "eq_claim",
                "{}:{}. Moving to this address is not allowed",
                file!(),
                line!(),
            );
            T::MoveClaimOrigin::try_origin(origin)
                .map(|_| ())
                .or_else(ensure_root)?;

            eq_ensure!(
                <Claims<T>>::get(&new).is_none(),
                Error::<T>::InvalidReceiver,
                target: "eq_claim",
                "{}:{}. Several claims are not allowed",
                file!(),
                line!(),
            );
            Claims::<T>::take(&old).map(|c| Claims::<T>::insert(&new, c));
            eq_ensure!(
                <Vesting<T>>::get(&new).is_none(),
                Error::<T>::InvalidReceiver,
                target: "eq_claim",
                "{}:{}. Several vestings are not allowed",
                file!(),
                line!(),
            );
            Vesting::<T>::take(&old).map(|c| Vesting::<T>::insert(&new, c));
            let s = <Signing<T>>::take(&old);
            <Signing<T>>::insert(&new, s);
            maybe_preclaim.map(|preclaim| {
                Preclaims::<T>::mutate(&preclaim, |maybe_o| {
                    if maybe_o.as_ref().map_or(false, |o| o == &old) {
                        *maybe_o = Some(new)
                    }
                })
            });

            Ok(().into())
        }
    }
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            let (maybe_signer, maybe_statement) = match call {
                // <weight>
                // Base Weight: 188.7 µs (includes the full logic of `validate_unsigned`)
                // DB Weight: 2 Read (Claims, Signing)
                // </weight>
                Call::claim {
                    dest: account,
                    ethereum_signature,
                } => {
                    let data = account.using_encoded(to_ascii_hex);
                    (Self::eth_recover(&ethereum_signature, &data, &[][..]), None)
                }
                // <weight>
                // Base Weight: 190.1 µs (includes the full logic of `validate_unsigned`)
                // DB Weight: 2 Read (Claims, Signing)
                // </weight>
                Call::claim_attest {
                    dest: account,
                    ethereum_signature,
                    statement,
                } => {
                    let data = account.using_encoded(to_ascii_hex);
                    (
                        Self::eth_recover(&ethereum_signature, &data, &statement),
                        Some(statement.as_slice()),
                    )
                }
                _ => {
                    log::error!("{}:{}. Call didn't match claim options", file!(), line!());
                    return Err(InvalidTransaction::Call.into());
                }
            };

            let signer = ok_or_error!(
                maybe_signer,
                InvalidTransaction::Custom(ValidityError::InvalidEthereumSignature.into()),
                "{}:{}. Invalid Ethereum signature. Signature: {:?}.",
                file!(),
                line!(),
                maybe_signer
            )?;

            let e = InvalidTransaction::Custom(ValidityError::SignerHasNoClaim.into());
            eq_ensure!(
                <Claims<T>>::contains_key(&signer),
                e,
                target: "eq_claim",
                "{}:{}. Signer has no claim. Who: {:?}.",
                file!(),
                line!(),
                signer
            );

            let e = InvalidTransaction::Custom(ValidityError::InvalidStatement.into());
            let s = <Signing<T>>::get(signer);
            if s {
                eq_ensure!(Some(get_statement_text()) == maybe_statement, e,
                    target: "eq_claim",
                    "{}:{}. Get_statement_text() not equal to statement from params. Get statement text: {:?}, from params: {:?}.",
                    file!(), line!(), get_statement_text(), maybe_statement);
            } else {
                eq_ensure!(
                    maybe_statement.is_none(),
                    e,
                    target: "eq_claim",
                    "{}:{}. Statement is none",
                    file!(),
                    line!()
                );
            }

            let priority = T::UnsignedPriority::get();

            Ok(ValidTransaction {
                priority,
                requires: vec![],
                provides: vec![("claims", signer).encode()],
                longevity: TransactionLongevity::max_value(),
                propagate: true,
            })
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub fn deposit_event)]
    pub enum Event<T: Config> {
        /// `AccountId` claimed `Balance` amount of currency reserved for `EthereumAddress`
        /// \[who, ethereum_account, amount\]
        Claimed(T::AccountId, EthereumAddress, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Invalid Ethereum signature
        InvalidEthereumSignature,
        /// Ethereum address has no claim
        SignerHasNoClaim,
        /// Account sending transaction has no claim
        SenderHasNoClaim,
        /// There's not enough in the pot to pay out some unvested amount. Generally
        /// implies a logic error
        PotUnderflow,
        /// A needed statement was not included.
        InvalidStatement,
        /// The account already has a vested balance
        VestedBalanceExists,
        /// This method is not allowed in production
        MethodNotAllowed,
        /// Invalid receiver
        InvalidReceiver,
    }

    /// Pallet storage - stores amount to be claimed by each `EthereumAddress`
    #[pallet::storage]
    #[pallet::getter(fn claims)]
    pub type Claims<T: Config> = StorageMap<_, Identity, EthereumAddress, BalanceOf<T>>;

    /// Pallet storage - total `Claims` amount
    #[pallet::storage]
    #[pallet::getter(fn total)]
    pub type Total<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Pallet storage - vesting schedule for a claim.
    /// First balance is the total amount that should be held for vesting.
    /// Second balance is how much should be unlocked per block.
    /// The block number is when the vesting should start.
    #[pallet::storage]
    #[pallet::getter(fn vesting)]
    pub type Vesting<T: Config> =
        StorageMap<_, Identity, EthereumAddress, (BalanceOf<T>, BalanceOf<T>, T::BlockNumber)>;

    /// Pallet storage - stores Ethereum addresses from which additional statement
    /// singing is required
    #[pallet::storage]
    pub type Signing<T: Config> = StorageMap<_, Identity, EthereumAddress, bool, ValueQuery>;

    /// Pallet storage - pre-claimed Ethereum accounts, by the Account ID that
    /// they are claimed to
    #[pallet::storage]
    pub type Preclaims<T: Config> = StorageMap<_, Identity, T::AccountId, EthereumAddress>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        #[doc = " Pallet storage - vesting schedule for a claim."]
        #[doc = " First balance is the total amount that should be held for vesting."]
        #[doc = " Second balance is how much should be unlocked per block."]
        #[doc = " The block number is when the vesting should start."]
        pub vesting: Vec<(
            EthereumAddress,
            (BalanceOf<T>, BalanceOf<T>, T::BlockNumber),
        )>,
        pub claims: Vec<(EthereumAddress, BalanceOf<T>, Option<T::AccountId>, bool)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                vesting: Default::default(),
                claims: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            {
                let builder: fn(&Self) -> _ = |config: &GenesisConfig<T>| {
                    config
                        .claims
                        .iter()
                        .map(|(a, b, _, _)| (*a, *b))
                        .collect::<Vec<_>>()
                };
                let data = &builder(self);
                let data: &frame_support::sp_std::vec::Vec<(EthereumAddress, BalanceOf<T>)> = data;
                data.iter().for_each(|(k, v)| {
                    <Claims<T> as frame_support::storage::StorageMap<
                        EthereumAddress,
                        BalanceOf<T>,
                    >>::insert::<&EthereumAddress, &BalanceOf<T>>(k, v);
                });
            }
            {
                let builder: fn(&Self) -> _ = |config: &GenesisConfig<T>| {
                    use sp_runtime::traits::Zero;
                    config
                        .claims
                        .iter()
                        .fold(Zero::zero(), |acc: BalanceOf<T>, &(_, b, _, _)| acc + b)
                };
                let data = &builder(self);
                let v: &BalanceOf<T> = data;
                <Total<T> as frame_support::storage::StorageValue<BalanceOf<T>>>::put::<
                    &BalanceOf<T>,
                >(v);
            }
            {
                let data = &self.vesting;
                let data: &frame_support::sp_std::vec::Vec<(
                    EthereumAddress,
                    (BalanceOf<T>, BalanceOf<T>, T::BlockNumber),
                )> = data;
                data.iter().for_each(|(k, v)| {
                    <Vesting<T> as frame_support::storage::StorageMap<
                        EthereumAddress,
                        (BalanceOf<T>, BalanceOf<T>, T::BlockNumber),
                    >>::insert::<&EthereumAddress, &(BalanceOf<T>, BalanceOf<T>, T::BlockNumber)>(
                        k, v,
                    );
                });
            }
            {
                let builder: fn(&Self) -> _ = |config: &GenesisConfig<T>| {
                    config
                        .claims
                        .iter()
                        .map(|(a, _, _, s)| (*a, *s))
                        .collect::<Vec<_>>()
                };
                let data = &builder(self);
                let data: &frame_support::sp_std::vec::Vec<(EthereumAddress, bool)> = data;
                data.iter().for_each(|(k, v)| {
                    <Signing<T> as frame_support::storage::StorageMap<EthereumAddress, bool>>
                        ::insert::<& EthereumAddress, &bool>(k, v);
                });
            }
            {
                let builder: fn(&Self) -> _ = |config: &GenesisConfig<T>| {
                    config
                        .claims
                        .iter()
                        .filter_map(|(a, _, i, _)| Some((i.clone()?, *a)))
                        .collect::<Vec<_>>()
                };
                let data = &builder(self);
                let data: &frame_support::sp_std::vec::Vec<(T::AccountId, EthereumAddress)> = data;
                data.iter().for_each(|(k, v)| {
                    <Preclaims<T> as frame_support::storage::StorageMap<
                        T::AccountId,
                        EthereumAddress,
                    >>::insert::<&T::AccountId, &EthereumAddress>(k, v);
                });
            }
        }
    }
}

/// An Ethereum address (i.e. 20 bytes, used to represent an Ethereum account).
///
/// This gets serialized to the 0x-prefixed hex representation.
#[derive(
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Encode,
    Decode,
    Default,
    RuntimeDebug,
    Hash,
    scale_info::TypeInfo,
)]
pub struct EthereumAddress([u8; 20]);

impl EthereumAddress {
    fn zero() -> Self {
        Self([0; 20])
    }
}

#[cfg(feature = "std")]
impl Serialize for EthereumAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex: String = rustc_hex::ToHex::to_hex(&self.0[..]);
        serializer.serialize_str(&format!("0x{}", hex))
    }
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for EthereumAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let base_string = String::deserialize(deserializer)?;
        let offset = if base_string.starts_with("0x") { 2 } else { 0 };
        let s = &base_string[offset..];
        if s.len() != 40 {
            log::error!(
                "{}:{}. Bad length of Ethereum address. Length: {:?}",
                file!(),
                line!(),
                s.len()
            );
            return Err(serde::de::Error::custom(
                "Bad length of Ethereum address (should be 42 including '0x')",
            ));
        }
        let raw: Vec<u8> = rustc_hex::FromHex::from_hex(s).map_err(|e| {
            log::error!("{}:{}. Couldn't convert from hex.", file!(), line!());
            serde::de::Error::custom(format!("{:?}", e))
        })?;
        let mut r = Self::default();
        r.0.copy_from_slice(&raw);
        Ok(r)
    }
}

#[derive(Encode, Decode, Clone, scale_info::TypeInfo)]
pub struct EcdsaSignature(pub [u8; 65]);

impl AsRef<[u8; 65]> for EcdsaSignature {
    fn as_ref(&self) -> &[u8; 65] {
        &self.0
    }
}

impl AsRef<[u8]> for EcdsaSignature {
    fn as_ref(&self) -> &[u8] {
        &self.0[..]
    }
}

impl PartialEq for EcdsaSignature {
    fn eq(&self, other: &Self) -> bool {
        self.0[..] == other.0[..]
    }
}

impl sp_std::fmt::Debug for EcdsaSignature {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        write!(f, "EcdsaSignature({:?})", &self.0[..])
    }
}

/// Convert this to the (English) statement it represents
pub fn get_statement_text() -> &'static [u8] {
    &b"I hereby agree to the terms of the statement whose SHA-256 hash is \
        c09a97ac5967e159719e3798c1d77e401b61c59e2735897ccfe4d9f6f9c4a8b2. (This may be found at the URL: \
        https://equilibrium.io/tokenswap/docs/token_swap_t&cs.pdf)"[..]
}

/// Converts the given binary data into ASCII-encoded hex. It will be twice the length
pub fn to_ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut r = Vec::with_capacity(data.len() * 2);
    let mut push_nibble = |n| r.push(if n < 10 { b'0' + n } else { b'a' - 10 + n });
    for &b in data.iter() {
        push_nibble(b / 16);
        push_nibble(b % 16);
    }
    r
}

impl<T: Config> Pallet<T> {
    // Constructs the message that Ethereum RPC's `personal_sign` and `eth_sign` would sign
    fn ethereum_signable_message(what: &[u8], extra: &[u8]) -> Vec<u8> {
        let prefix = T::Prefix::get();
        let mut l = prefix.len() + what.len() + extra.len();
        let mut rev = Vec::new();
        while l > 0 {
            rev.push(b'0' + (l % 10) as u8);
            l /= 10;
        }
        let mut v = b"\x19Ethereum Signed Message:\n".to_vec();
        v.extend(rev.into_iter().rev());
        v.extend_from_slice(&prefix[..]);
        v.extend_from_slice(what);
        v.extend_from_slice(extra);
        v
    }

    // Attempts to recover the Ethereum address from a message signature signed by using
    // the Ethereum RPC's `personal_sign` and `eth_sign`
    fn eth_recover(s: &EcdsaSignature, what: &[u8], extra: &[u8]) -> Option<EthereumAddress> {
        let msg = keccak_256(&Self::ethereum_signable_message(what, extra));
        let mut res = EthereumAddress::default();
        res.0
            .copy_from_slice(&keccak_256(&secp256k1_ecdsa_recover(&s.0, &msg).ok()?[..])[12..]);
        Some(res)
    }

    fn process_claim(signer: EthereumAddress, dest: T::AccountId) -> DispatchResult {
        let option_balance_of = <Claims<T>>::get(&signer);
        let balance_due = ok_or_error!(
            option_balance_of,
            Error::<T>::SignerHasNoClaim,
            "{}:{}. Signer has no claim. Address: {:?}.",
            file!(),
            line!(),
            signer
        )?;

        let option_checked = Self::total().checked_sub(&balance_due);
        let new_total = ok_or_error!(option_checked, Error::<T>::PotUnderflow,
        "{}:{}. Not enough in the pot to pay out some unvested amount. Total: {:?}, balanceOf: {:?}, address: {:?}", 
        file!(), line!(), Self::total(), balance_due, signer)?;

        let vesting = Vesting::<T>::get(&signer);
        if vesting.is_some() && T::VestingSchedule::vesting_balance(&dest).is_some() {
            return Err({
                log::error!("{}:{}. The account already has a vested balance. Who ID: {:?}, dest ethereum address: {:?}.", 
            file!(), line!(), dest, signer);
                Error::<T>::VestedBalanceExists.into()
            });
        }

        // Check if this claim should have a vesting schedule.
        if let Some(vs) = vesting {
            let initial_balance = balance_due.saturating_sub(vs.0);
            CurrencyOf::<T>::deposit_creating(&dest, initial_balance);
            let vesting_account_id = T::VestingAccountId::get();

            CurrencyOf::<T>::deposit_creating(&vesting_account_id, vs.0);

            // This can only fail if the account already has a vesting schedule,
            // but this is checked above.
            T::VestingSchedule::add_vesting_schedule(&dest, vs.0, vs.1, vs.2)
                .expect("No other vesting schedule exists, as checked above; qed");
        } else {
            CurrencyOf::<T>::deposit_creating(&dest, balance_due);
        }

        <Total<T>>::put(new_total);
        <Claims<T>>::remove(&signer);
        <Vesting<T>>::remove(&signer);
        <Signing<T>>::remove(&signer);

        // Let's deposit an event to let the outside world know this happened.
        Self::deposit_event(Event::Claimed(dest, signer, balance_due));

        Ok(())
    }
}

/// Validate `attest` calls prior to execution. Needed to avoid a DoS attack since they are
/// otherwise free to place on chain.
#[derive(Encode, Decode, Clone, Eq, PartialEq, scale_info::TypeInfo)]
pub struct PrevalidateAttests<T: Config + Send + Sync + scale_info::TypeInfo>(
    sp_std::marker::PhantomData<T>,
)
where
    <T as frame_system::Config>::Call: IsSubType<Call<T>>;

impl<T: Config + Send + Sync + scale_info::TypeInfo> Debug for PrevalidateAttests<T>
where
    <T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "PrevalidateAttests")
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> Default for PrevalidateAttests<T>
where
    <T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
    fn default() -> Self {
        Self(sp_std::marker::PhantomData)
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> PrevalidateAttests<T>
where
    <T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
    /// Create new `SignedExtension` to check runtime version.
    pub fn new() -> Self {
        Self(sp_std::marker::PhantomData)
    }
}

impl<T: Config + Send + Sync + scale_info::TypeInfo> SignedExtension for PrevalidateAttests<T>
where
    <T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
    type AccountId = T::AccountId;
    type Call = <T as frame_system::Config>::Call;
    type AdditionalSigned = ();
    type Pre = ();
    const IDENTIFIER: &'static str = "PrevalidateAttests";

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }

    // <weight>
    // Base Weight: 8.631 µs
    // DB Weight: 2 Read (Preclaims, Signing)
    // </weight>
    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> TransactionValidity {
        if let Some(local_call) = call.is_sub_type() {
            if let Call::attest {
                statement: attested_statement,
            } = local_call
            {
                let option_ethereum_address = Preclaims::<T>::get(who);
                let signer = ok_or_error!(
                    option_ethereum_address,
                    InvalidTransaction::Custom(ValidityError::SignerHasNoClaim.into()),
                    "{}:{}. Signer has no claim. Who: {:?}.",
                    file!(),
                    line!(),
                    who
                )?;
                let s = <Signing<T>>::get(signer);
                if s {
                    let e = InvalidTransaction::Custom(ValidityError::InvalidStatement.into());
                    eq_ensure!(&attested_statement[..] == get_statement_text(), e,
                    target: "eq_claim",
                    "{}:{}. Get_statement_text() not equal to statement from call. Get statement text: {:?}, from call: {:?}.",
                    file!(), line!(), get_statement_text(), &attested_statement[..]);
                }
            }
        }
        Ok(ValidTransaction::default())
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        self.validate(who, call, info, len)
            .map(|_| Self::Pre::default())
            .map_err(Into::into)
    }
}
