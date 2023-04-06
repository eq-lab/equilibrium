use eq_primitives::asset::Asset;
use eq_primitives::{asset, Aggregates, SignedBalance, TotalAggregates, UserGroup};
use sp_runtime::DispatchResult;
use sp_std::boxed::Box;

pub struct AggregatesMock;

impl Aggregates<u64, u64> for AggregatesMock {
    fn in_usergroup(_account_id: &u64, _user_group: UserGroup) -> bool {
        true
    }
    fn set_usergroup(_account_id: &u64, _user_group: UserGroup, _is_in: bool) -> DispatchResult {
        Ok(())
    }

    fn update_total(
        _account_id: &u64,
        _currency: Asset,
        _prev_balance: &SignedBalance<u64>,
        _delta_balance: &SignedBalance<u64>,
    ) -> DispatchResult {
        Ok(())
    }

    fn iter_account(_user_group: UserGroup) -> Box<dyn Iterator<Item = u64>> {
        panic!("AggregatesMock not implemented");
    }
    fn iter_total(
        _user_group: UserGroup,
    ) -> Box<dyn Iterator<Item = (asset::Asset, TotalAggregates<u64>)>> {
        panic!("AggregatesMock not implemented");
    }
    fn get_total(_user_group: UserGroup, _currency: Asset) -> TotalAggregates<u64> {
        panic!("AggregatesMock not implemented");
    }
}
