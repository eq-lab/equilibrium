#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn create() -> Weight;
    fn purge() -> Weight;
    fn deposit() -> Weight;
    fn withdraw() -> Weight;
    fn claim() -> Weight;
    fn claim_other() -> Weight;
    fn on_initialize() -> Weight;
    fn set_last_id() -> Weight;
    fn set_manager() -> Weight;
}

impl WeightInfo for () {
    fn create() -> Weight {
        Weight::zero()
    }

    fn purge() -> Weight {
        Weight::zero()
    }

    fn deposit() -> Weight {
        Weight::zero()
    }

    fn withdraw() -> Weight {
        Weight::zero()
    }

    fn claim() -> Weight {
        Weight::zero()
    }

    fn claim_other() -> Weight {
        Weight::zero()
    }

    fn on_initialize() -> Weight {
        Weight::zero()
    }

    fn set_last_id() -> Weight {
        Weight::zero()
    }

    fn set_manager() -> Weight {
        Weight::zero()
    }
}
