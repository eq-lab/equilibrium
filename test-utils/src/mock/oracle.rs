#[macro_export]
/// Generate struct OracleMock
/// Price getter and price setter with default price and thread local storage
// Example 1 (only price setter with default price FixedI64::one()):
//     test_utils::generate_oracle_mock!();
// Example 2 (with thread_local storage price getter and setter):
//     thread_local! {
//         static PRICES: RefCell<Vec<(Asset, FixedI64)>> = RefCell::new(vec![
//             (asset::BTC, FixedI64::saturating_from_integer(10000)),
//             (asset::EOS, FixedI64::saturating_from_integer(3)),
//             (asset::ETH, FixedI64::saturating_from_integer(250)),
//             (asset::EQD, FixedI64::saturating_from_integer(1)),
//             (asset::EQ, FixedI64::saturating_from_integer(1)),
//             (asset::GENS, FixedI64::saturating_from_integer(1))
//         ]);
//     }
//
//     test_utils::generate_oracle_mock!(PRICES, FixedI64::zero());
//
macro_rules! generate_oracle_mock {
    () => {
        pub struct OracleMock;

        impl eq_primitives::PriceGetter for OracleMock {
            fn get_price(
                _currency: &eq_primitives::asset::Asset,
            ) -> Result<FixedI64, sp_runtime::DispatchError> {
                Ok(sp_arithmetic::FixedI64::one())
            }
        }
    };

    ($default_price: expr) => {
        pub struct OracleMock;

        impl eq_primitives::PriceGetter for OracleMock {
            fn get_price(
                _currency: &Asset,
            ) -> Result<sp_arithmetic::FixedI64, sp_runtime::DispatchError> {
                Ok($default_price)
            }
        }
    };

    ($prices:expr, $default_price: expr) => {
        pub struct OracleMock;

        pub trait PriceSetter {
            fn set_price_mock(
                currency: &eq_primitives::asset::Asset,
                value: &sp_arithmetic::FixedI64,
            );
        }

        impl PriceSetter for OracleMock {
            fn set_price_mock(currency: &Asset, value: &sp_arithmetic::FixedI64) {
                $prices.with(|v| {
                    let mut vec = v.borrow().clone();
                    for pair in vec.iter_mut() {
                        if pair.0 == *currency {
                            pair.1 = value.clone();
                        }
                    }

                    *v.borrow_mut() = vec;
                });
            }
        }

        impl eq_primitives::PriceGetter for OracleMock {
            fn get_price(
                currency: &eq_primitives::asset::Asset,
            ) -> Result<sp_arithmetic::FixedI64, sp_runtime::DispatchError> {
                let mut return_value = $default_price;
                $prices.with(|v| {
                    let value = v.borrow().clone();
                    for pair in value.iter() {
                        if pair.0 == *currency {
                            return_value = pair.1.clone();
                        }
                    }
                });
                return Ok(return_value);
            }
        }
    };
}
