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

#![cfg(feature = "runtime-benchmarks")]
use super::*;
use crate::secp_utils::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_system::RawOrigin;
use sp_runtime::traits::ValidateUnsigned;

const SEED: u32 = 0;

const MAX_CLAIMS: u32 = 10_000;
const VALUE: u32 = 1_000_000;

fn create_claim<T: Config>(input: u32) -> DispatchResultWithPostInfo {
    let secret_key = secp256k1::SecretKey::parse(&keccak_256(&input.encode())).unwrap();
    let eth_address = eth(&secret_key);
    let vesting = Some((100_000u32.into(), 1_000u32.into(), 100u32.into()));
    super::Pallet::<T>::mint_claim(
        RawOrigin::Root.into(),
        eth_address,
        VALUE.into(),
        vesting,
        false,
    )?;
    Ok(().into())
}

fn create_claim_attest<T: Config>(input: u32) -> DispatchResultWithPostInfo {
    let secret_key = secp256k1::SecretKey::parse(&keccak_256(&input.encode())).unwrap();
    let eth_address = eth(&secret_key);
    let vesting = Some((100_000u32.into(), 1_000u32.into(), 100u32.into()));
    super::Pallet::<T>::mint_claim(
        RawOrigin::Root.into(),
        eth_address,
        VALUE.into(),
        vesting,
        true,
    )?;
    Ok(().into())
}

benchmarks! {
    // Benchmark `claim` for different users.
    claim {
        let c = MAX_CLAIMS;

        for i in 0 .. c / 2 {
            create_claim::<T>(i)?;
            create_claim_attest::<T>(u32::max_value() - i)?;
        }
        let secret_key = secp256k1::SecretKey::parse(&keccak_256(&c.encode())).unwrap();
        let eth_address = eth(&secret_key);
        let account: T::AccountId = account("user", c, SEED);
        let vesting = Some((100_000u32.into(), 1_000u32.into(), 100u32.into()));
        let signature = sig::<T>(&secret_key, &account.encode(), &[][..]);
        super::Pallet::<T>::mint_claim(RawOrigin::Root.into(), eth_address, VALUE.into(), vesting, false)?;
        assert_eq!(Claims::<T>::get(eth_address), Some(VALUE.into()));
        let source = sp_runtime::transaction_validity::TransactionSource::External;
        let call = crate::Call::<T>::claim{dest: account.clone(),  ethereum_signature: signature.clone()};
    }: {
        super::Pallet::<T>::validate_unsigned(source, &call).unwrap();
        super::Pallet::<T>::claim(RawOrigin::None.into(), account, signature).unwrap();
    }
    verify {
        assert_eq!(Claims::<T>::get(eth_address), None);
    }

    // Benchmark `mint_claim` when there already exists `c` claims in storage.
    mint_claim {
        let c = MAX_CLAIMS;

        for i in 0 .. c / 2 {
            create_claim::<T>(i)?;
            create_claim_attest::<T>(u32::max_value() - i)?;
        }
        let eth_address = account("eth_address", 42, SEED);
        let vesting = Some((100_000u32.into(), 1_000u32.into(), 100u32.into()));
        let statement = true;
    }: _(RawOrigin::Root, eth_address, VALUE.into(), vesting, statement)
    verify {
        assert_eq!(Claims::<T>::get(eth_address), Some(VALUE.into()));
    }

    // Benchmark `claim_attest` for different users.
    claim_attest {
        let c = MAX_CLAIMS;

        for i in 0 .. c / 2 {
            create_claim::<T>(i)?;
            create_claim_attest::<T>(u32::max_value() - i)?;
        }
        let attest_u = u32::max_value() - c;
        let secret_key = secp256k1::SecretKey::parse(&keccak_256(&attest_u.encode())).unwrap();
        let eth_address = eth(&secret_key);
        let account: T::AccountId = account("user", c, SEED);
        let vesting = Some((100_000u32.into(), 1_000u32.into(), 100u32.into()));
        let statement = true;
        let signature = sig::<T>(&secret_key, &account.encode(), get_statement_text());
        super::Pallet::<T>::mint_claim(RawOrigin::Root.into(), eth_address, VALUE.into(), vesting, statement)?;
        assert_eq!(Claims::<T>::get(eth_address), Some(VALUE.into()));
        let call = crate::Call::<T>::claim_attest{dest: account.clone(), ethereum_signature: signature.clone(), statement: get_statement_text().to_vec()};
        let source = sp_runtime::transaction_validity::TransactionSource::External;
    }: {
        super::Pallet::<T>::validate_unsigned(source, &call).unwrap();
        super::Pallet::<T>::claim_attest(RawOrigin::None.into(), account, signature, get_statement_text().to_vec()).unwrap();
    }
    verify {
        assert_eq!(Claims::<T>::get(eth_address), None);
    }

    // Benchmark `attest` including prevalidate logic.
        attest {
            let c = MAX_CLAIMS;

            for i in 0 .. c / 2 {
                create_claim::<T>(i)?;
                create_claim_attest::<T>(u32::max_value() - i)?;
            }

            let attest_c = u32::max_value() - c;
            let secret_key = secp256k1::SecretKey::parse(&keccak_256(&attest_c.encode())).unwrap();
            let eth_address = eth(&secret_key);
            let account: T::AccountId = account("user", c, SEED);
            let vesting = Some((100_000u32.into(), 1_000u32.into(), 100u32.into()));
            let signature = sig::<T>(&secret_key, &account.encode(), get_statement_text());
            super::Pallet::<T>::mint_claim(RawOrigin::Root.into(), eth_address, VALUE.into(), vesting, true)?;
            Preclaims::<T>::insert(&account, eth_address);
            assert_eq!(Claims::<T>::get(eth_address), Some(VALUE.into()));
        }: {
            super::Pallet::<T>::attest(RawOrigin::Signed(account).into(), get_statement_text().to_vec()).unwrap();
        }
        verify {
            assert_eq!(Claims::<T>::get(eth_address), None);
        }
    // Benchmark the time it takes to do `repeat` number of keccak256 hashes
    #[extra]
    keccak256 {
        let i in 0 .. 10_000;
        let bytes = (i).encode();
    }: {
        for index in 0 .. i {
            let _hash = keccak_256(&bytes);
        }
    }

    // Benchmark the time it takes to do `repeat` number of `eth_recover`
    #[extra]
    eth_recover {
        let i in 0 .. 1_000;
        // Crate signature
        let secret_key = secp256k1::SecretKey::parse(&keccak_256(&i.encode())).unwrap();
        let account: T::AccountId = account("user", i, SEED);
        let signature = sig::<T>(&secret_key, &account.encode(), &[][..]);
        let data = account.using_encoded(to_ascii_hex);
        let extra = get_statement_text();
    }: {
        for _ in 0 .. i {
            assert!(super::Pallet::<T>::eth_recover(&signature, &data, extra).is_some());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::tests::{new_test_ext, Test};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        new_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_claim::<Test>());
            assert_ok!(test_benchmark_mint_claim::<Test>());
            assert_ok!(test_benchmark_claim_attest::<Test>());
            assert_ok!(test_benchmark_attest::<Test>());
            assert_ok!(test_benchmark_keccak256::<Test>());
            assert_ok!(test_benchmark_eth_recover::<Test>());
        });
    }
}
