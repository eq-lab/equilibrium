use sp_runtime::DispatchResult;

/// A vesting schedule over a currency. This allows a particular currency to have vesting limits
/// applied to it.
pub trait EqVestingSchedule<Balance, AccountId> {
    /// The quantity used to denote time; usually just a `BlockNumber`.
    type Moment;

    /// Get the amount that is currently being vested and cannot be transferred out of this account.
    /// Returns `None` if the account has no vesting schedule.
    fn vesting_balance(who: &AccountId) -> Option<Balance>;

    /// Adds a vesting schedule to a given account.
    ///
    /// If the account has `MaxVestingSchedules`, an Error is returned and nothing
    /// is updated.
    ///
    /// Is a no-op if the amount to be vested is zero.
    ///
    /// NOTE: This doesn't alter the free balance of the account.
    fn add_vesting_schedule(
        who: &AccountId,
        locked: Balance,
        per_block: Balance,
        starting_block: Self::Moment,
    ) -> DispatchResult;

    /// Checks if `add_vesting_schedule` would work against `who`.
    fn can_add_vesting_schedule(
        who: &AccountId,
        locked: Balance,
        per_block: Balance,
        starting_block: Self::Moment,
    ) -> DispatchResult;

    /// Remove a vesting schedule for a given account.
    ///
    /// NOTE: This doesn't alter the free balance of the account.
    fn remove_vesting_schedule(who: &AccountId, schedule_index: u32) -> DispatchResult;
}
