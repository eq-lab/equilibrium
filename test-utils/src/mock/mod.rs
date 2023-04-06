use eq_primitives::asset::Asset;
use eq_primitives::EqBuyout;

pub mod aggregates;
pub mod financial;
pub mod oracle;

pub struct EqBuyoutMock;
impl<AccountId, Balance> EqBuyout<AccountId, Balance> for EqBuyoutMock {
    fn eq_buyout(_who: &AccountId, _amount: Balance) -> sp_runtime::DispatchResult {
        Ok(())
    }
    fn is_enough(
        _asset: Asset,
        _amount: Balance,
        _amount_buyout: Balance,
    ) -> Result<bool, sp_runtime::DispatchError> {
        Ok(true)
    }
}
