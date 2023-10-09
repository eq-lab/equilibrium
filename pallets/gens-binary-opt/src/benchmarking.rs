#![cfg(feature = "runtime-benchmarks")]
pub use super::*;
use eq_utils::ONE_TOKEN;
use frame_benchmarking::{/*account,*/ benchmarks, whitelisted_caller, Zero};
use frame_support::traits::OnInitialize;
use frame_system::RawOrigin;

pub const PROPER_ASSET: Asset = eq_primitives::asset::GENS;
pub const TARGET_ASSET: Asset = eq_primitives::asset::BTC;
pub const BINARY_ID: u64 = 0;
pub const FIVE_SECONDS: u64 = 5;
pub struct Pallet<T: Config>(crate::Pallet<T>);

pub trait Config: crate::Config + pallet_timestamp::Config {}

fn create_binary<T: Config>() {
    pallet::Pallet::<T>::create(
        RawOrigin::Root.into(),
        BINARY_ID.into(),
        FIVE_SECONDS.into(),
        0,
        TARGET_ASSET,
        BinaryMode::CallPut(FixedI64::zero()),
        PROPER_ASSET,
        ONE_TOKEN.into(),
        Permill::from_parts(50_000),
        Permill::from_parts(50_000),
    )
    .unwrap();
}

fn deposit_call<T: Config>(caller: T::AccountId, amount: T::Balance, vote: bool) {
    add_money::<T>(&caller, amount);
    pallet::Pallet::<T>::deposit(
        RawOrigin::Signed(caller).into(),
        BINARY_ID.into(),
        vote,
        amount,
    )
    .unwrap();
}

fn add_money<T: Config>(caller: &T::AccountId, amount: T::Balance) {
    T::EqCurrency::deposit_creating(PROPER_ASSET, caller, amount, true, None).unwrap();
}

fn time_move<T: Config>(moment: u64) {
    pallet_timestamp::Pallet::<T>::set_timestamp((1000 * moment as u32).into());
}

// fn debug<T:Config> () {
//     panic!("{:?}", T::UnixTime::now());
// }

benchmarks! {
    create {}: _(
        RawOrigin::Root,
        BINARY_ID.into(),
        FIVE_SECONDS.into(),
        0,
        TARGET_ASSET,
        BinaryMode::CallPut(FixedI64::zero()),
        PROPER_ASSET,
        ONE_TOKEN.into(),
        Permill::from_parts(50_000),
        Permill::from_parts(50_000)
    )
    verify {
        assert!(Binaries::<T>::get::<T::BinaryId>(BINARY_ID.into()).is_some());
    }

    purge {
        create_binary::<T>();
        let caller: T::AccountId = whitelisted_caller();
        let deposit_amount =  (2 * ONE_TOKEN).into();
        // worst case scenario: there are no winners
        deposit_call::<T>(caller.clone(), deposit_amount, false);
        time_move::<T>(FIVE_SECONDS);
        pallet::Pallet::<T>::on_initialize(T::BlockNumber::zero());
    }: _(
        RawOrigin::Signed(caller),
        BINARY_ID.into()
    )
    verify {
        assert!(Binaries::<T>::get::<T::BinaryId>(BINARY_ID.into()).is_none());
        // making sure there were no winners and all the money has gone to treasury
        assert_eq!(
            T::EqCurrency::total_balance(PROPER_ASSET, &T::TreasuryModuleId::get().into_account_truncating()),
            (2 * ONE_TOKEN).into()
        );
    }

    deposit {
        create_binary::<T>();
        let caller = whitelisted_caller();
        let deposit_amount: T::Balance = (2 * ONE_TOKEN).into();
        add_money::<T>(&caller, deposit_amount);
    }: _(
        RawOrigin::Signed(caller),
        BINARY_ID.into(),
        true,
        deposit_amount
    )

    withdraw {
        create_binary::<T>();
        let caller: T::AccountId = whitelisted_caller();
        let deposit_amount = (2 * ONE_TOKEN).into();
        deposit_call::<T>(caller.clone(), deposit_amount, true);
    }: _(
        RawOrigin::Signed(caller),
        BINARY_ID.into()
    )

    on_initialize{
        create_binary::<T>();
        let caller: T::AccountId = whitelisted_caller();
        let deposit_amount =  (2 * ONE_TOKEN).into();
        deposit_call::<T>(caller.clone(), deposit_amount, true);
        time_move::<T>(FIVE_SECONDS);

        let block_number = T::BlockNumber::zero();
    }:{
        pallet::Pallet::<T>::on_initialize(block_number);
    }
    verify {
        let (binary, result, winners_left) = Binaries::<T>::get::<T::BinaryId>(BINARY_ID.into()).unwrap();
        assert!(result.is_some());
        assert_eq!(winners_left, 1);
    }

    claim {
        create_binary::<T>();
        let caller: T::AccountId = whitelisted_caller();
        let deposit_amount =  (2 * ONE_TOKEN).into();
        deposit_call::<T>(caller.clone(), deposit_amount, true);
        time_move::<T>(FIVE_SECONDS);
        pallet::Pallet::<T>::on_initialize(T::BlockNumber::zero());
    }: _(
        RawOrigin::Signed(caller),
        BINARY_ID.into()
    )

    claim_other {
        create_binary::<T>();
        let caller: T::AccountId = whitelisted_caller();
        let deposit_amount =  (2 * ONE_TOKEN).into();
        deposit_call::<T>(caller.clone(), deposit_amount, true);
        time_move::<T>(FIVE_SECONDS);
        pallet::Pallet::<T>::on_initialize(T::BlockNumber::zero());
        let another_caller: T::AccountId = whitelisted_caller();
    }: _(
        RawOrigin::Signed(another_caller),
        caller,
        BINARY_ID.into()
    )
}
