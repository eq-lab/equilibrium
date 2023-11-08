use sp_runtime::DispatchResult;

/// A vesting schedule over a currency. This allows a particular currency to have vesting limits
/// applied to it.
pub trait EqVestingSchedule<Balance, AccountId> {
    /// The quantity used to denote time; usually just a `BlockNumber`.
    type Moment;

    /// Get the amount that is currently being vested and cannot be transferred out of this account.
    fn vesting_balance(who: &AccountId) -> Option<Balance>;

    /// Adds a vesting schedule to a given account.
    fn add_vesting_schedule(
        who: &AccountId,
        locked: Balance,
        per_block: Balance,
        starting_block: Self::Moment,
    ) -> DispatchResult;

    /// Updates an existings vesting schedule for a given account.
    fn update_vesting_schedule(
        who: &AccountId,
        locked: Balance,
        duration_blocks: Balance,
    ) -> DispatchResult;
}
