#[macro_export]
/// Implement Financial, FinancialStorage, OnPriceSet traits for unit-tests Test Runtime
/// Required dependencies: financial_pallet, financial_primitives
// Example:
//      test_utils::implement_financial!()
macro_rules! implement_financial {
    () => {
        impl financial_pallet::Financial for Pallet<Test> {
            type Asset = eq_primitives::asset::Asset;
            type Price = substrate_fixed::types::I64F64;
            type AccountId = u64;
            fn calc_return(
                _return_type: financial_primitives::CalcReturnType,
                _asset: Self::Asset,
            ) -> Result<Vec<Self::Price>, DispatchError> {
                Ok(vec![])
            }
            fn calc_vol(
                _return_type: financial_primitives::CalcReturnType,
                _volatility_type: financial_primitives::CalcVolatilityType,
                _asset: Self::Asset,
            ) -> Result<Self::Price, DispatchError> {
                Ok(substrate_fixed::types::I64F64::from_num(0))
            }
            fn calc_corr(
                _return_type: financial_primitives::CalcReturnType,
                _correlation_type: financial_primitives::CalcVolatilityType,
                _asset1: Self::Asset,
                _asset2: Self::Asset,
            ) -> Result<(Self::Price, core::ops::Range<financial_pallet::Duration>), DispatchError>
            {
                Ok((
                    substrate_fixed::types::I64F64::from_num(0),
                    Default::default(),
                ))
            }
            fn calc_portf_vol(
                _return_type: financial_primitives::CalcReturnType,
                _vol_cor_type: financial_primitives::CalcVolatilityType,
                _account_id: Self::AccountId,
            ) -> Result<Self::Price, DispatchError> {
                Ok(substrate_fixed::types::I64F64::from_num(0))
            }
            fn calc_portf_var(
                _return_type: financial_primitives::CalcReturnType,
                _vol_cor_type: financial_primitives::CalcVolatilityType,
                _account_id: Self::AccountId,
                _z_score: u32,
            ) -> Result<Self::Price, DispatchError> {
                Ok(substrate_fixed::types::I64F64::from_num(0))
            }
            fn calc_rv(
                _return_type: financial_primitives::CalcReturnType,
                _ewma_length: u32,
                _asset: Self::Asset,
            ) -> Result<Self::Price, DispatchError> {
                Ok(substrate_fixed::types::I64F64::from_num(0))
            }
        }

        impl financial_primitives::OnPriceSet for Pallet<Test> {
            type Asset = eq_primitives::asset::Asset;
            type Price = substrate_fixed::types::I64F64;
            fn on_price_set(_asset: Self::Asset, _price: Self::Price) -> Result<(), DispatchError> {
                Ok(())
            }
        }
    };
}
