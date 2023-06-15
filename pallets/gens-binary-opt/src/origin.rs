use crate::{Config, PalletManager};
use frame_support::traits::EnsureOrigin;

pub struct EnsureManager<T, I: 'static = ()>(T, I);

impl<T: Config<I>, I: 'static> EnsureOrigin<T::RuntimeOrigin> for EnsureManager<T, I> {
    type Success = T::AccountId;

    fn try_origin(o: T::RuntimeOrigin) -> Result<Self::Success, T::RuntimeOrigin> {
        use frame_system::RawOrigin;
        use RawOrigin::Signed;
        o.into().and_then(|raw| match raw {
            Signed(ref acc_id) => match <PalletManager<T, I>>::get() {
                Some(manager_id) => {
                    if manager_id == *acc_id {
                        Ok(manager_id)
                    } else {
                        Err(T::RuntimeOrigin::from(raw))
                    }
                }
                None => Err(T::RuntimeOrigin::from(raw)),
            },
            r => Err(T::RuntimeOrigin::from(r)),
        })
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn successful_origin() -> T::RuntimeOrigin {
        todo!()
    }
}
