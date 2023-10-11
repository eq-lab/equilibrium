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

//! # Equilibrium ChainBridge Pallet
//!
//! Equilibrium's ChainBridge Pallet is a cross-chain bridge pallet for Equilibrium
//! substrate.
//!
//! We can divide extrinsics of this pallet into several groups:
//!
//! 1. Settings extrinsics:
//!     set_threshold, set_resource, remove_resource, whitelist_chain, add_relayer, remove_relayer:
//!     They allow to configure the number of confirmations that are required to complete the proposal,
//!     specify supported blockchains, register/unregister relays.
//!     Only registered relays may submit/sign proposals.
//!
//! 2. Proposal lifecycle extrinsics: relays use these and can either acknowledge_proposal or reject_proposal.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
// #![deny(warnings)]

mod benchmarking;
mod mock;
mod tests;
pub mod weights;

use codec::{Codec, Decode, Encode, EncodeLike};
use core::convert::TryInto;
use eq_primitives::asset::AssetGetter;
use eq_primitives::balance::BalanceGetter;
use eq_primitives::imbalances::{NegativeImbalance, PositiveImbalance};
use eq_primitives::signed_balance::SignedBalance;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::{
    dispatch::GetDispatchInfo,
    ensure,
    traits::{Currency, EnsureOrigin, ExistenceRequirement, Get},
    Parameter,
};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::{self as system, ensure_signed};
use sp_core::U256;
use sp_runtime::traits::{
    AccountIdConversion, AtLeast32BitUnsigned, Dispatchable, MaybeSerializeDeserialize, Member,
};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub use eq_primitives::chainbridge::*;

/// Default number of votes required for a proposal to execute
const DEFAULT_RELAYER_THRESHOLD: u32 = 1;
const DEFAULT_PROPOSAL_LIFETIME: u32 = 144000;

/// Helper function to concatenate a chain ID and some bytes to produce a resource ID.
/// The common format is (31 bytes unique ID + 1 byte chain ID).
pub fn derive_resource_id(chain: u8, id: &[u8]) -> ResourceId {
    let mut r_id: ResourceId = [0; 32];
    r_id[31] = chain; // last byte is chain id
    let range = if id.len() > 31 { 31 } else { id.len() }; // Use at most 31 bytes
    for i in 0..range {
        r_id[30 - i] = id[range - 1 - i]; // Ensure left padding for eth compatibility
    }
    return r_id;
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub enum ProposalStatus {
    Initiated,
    Approved,
    Rejected,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub struct ProposalVotes<AccountId, BlockNumber> {
    pub votes_for: Vec<AccountId>,
    pub votes_against: Vec<AccountId>,
    pub status: ProposalStatus,
    pub expiry: BlockNumber,
}

impl<A: PartialEq, B: PartialOrd + Default> ProposalVotes<A, B> {
    /// Attempts to mark the proposal as approve or rejected.
    /// Returns true if the status changes from active.
    fn try_to_complete(&mut self, threshold: u32, total: u32) -> ProposalStatus {
        if self.votes_for.len() >= threshold as usize {
            self.status = ProposalStatus::Approved;
            ProposalStatus::Approved
        } else if total >= threshold && self.votes_against.len() as u32 + threshold > total {
            self.status = ProposalStatus::Rejected;
            ProposalStatus::Rejected
        } else {
            ProposalStatus::Initiated
        }
    }

    /// Returns true if the proposal has been rejected or approved, otherwise false.
    fn is_complete(&self) -> bool {
        self.status != ProposalStatus::Initiated
    }

    /// Returns true if `who` has voted for or against the proposal
    fn has_voted(&self, who: &A) -> bool {
        self.votes_for.contains(&who) || self.votes_against.contains(&who)
    }

    /// Return true if the expiry time has been reached
    fn is_expired(&self, now: B) -> bool {
        self.expiry <= now
    }
}

impl<AccountId, BlockNumber: Default> Default for ProposalVotes<AccountId, BlockNumber> {
    fn default() -> Self {
        Self {
            votes_for: vec![],
            votes_against: vec![],
            status: ProposalStatus::Initiated,
            expiry: BlockNumber::default(),
        }
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    //use eq_utils::ONE_TOKEN;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + eq_assets::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Origin used to administer the pallet.
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Numerical representation of stored balances.
        type Balance: Parameter
            + Member
            + MaybeSerializeDeserialize
            + AtLeast32BitUnsigned
            + Default
            + Codec
            + Copy
            + Into<eq_primitives::balance::Balance>;

        /// The currency adapter trait.
        type Currency: Currency<
            Self::AccountId,
            Balance = Self::Balance,
            PositiveImbalance = PositiveImbalance<Self::Balance>,
            NegativeImbalance = NegativeImbalance<Self::Balance>,
        >;

        /// Gets balance of fee account.
        type BalanceGetter: BalanceGetter<Self::AccountId, Self::Balance>;

        /// The identifier for this chain.
        /// This must be unique and must not collide with existing IDs within a set of bridged chains.
        #[pallet::constant]
        type ChainIdentity: Get<ChainId>;

        /// Proposed dispatchable call.
        type Proposal: Parameter
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
            + EncodeLike
            + From<frame_system::Call<Self>>
            + GetDispatchInfo;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sets the vote threshold for proposals.
        ///
        /// This threshold is used to determine how many votes are required
        /// before a proposal is executed.
        ///
        /// # <weight>
        /// - O(1) lookup and insert
        /// # </weight>
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_threshold())]
        pub fn set_threshold(origin: OriginFor<T>, threshold: u32) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::set_relayer_threshold(threshold)
        }

        /// Sets fee for chain id.
        ///
        /// After that the transfers will get the fee from runtime storage.
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_fee(
            origin: OriginFor<T>,
            chain_id: ChainId,
            fee: T::Balance,
        ) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::set_chain_fee(chain_id, fee)
        }

        /// Sets proposal lifetime.
        ///
        /// This lifetime is used for determine how many blocks have relays to votes for deposit.
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_proposal_lifetime(
            origin: OriginFor<T>,
            lifetime: BlockNumberFor<T>,
        ) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::set_lifetime(lifetime)
        }

        /// Sets transfers allowability.
        ///
        /// This param is used for determine transfers possibility for chain.
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn toggle_chain(
            origin: OriginFor<T>,
            chain_id: ChainId,
            enabled: bool,
        ) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::toggle_chain_state(chain_id, enabled)
        }

        /// Stores a method name on chain under an associated resource ID.
        ///
        /// # <weight>
        /// - O(1) write
        /// # </weight>
        #[pallet::call_index(4)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_resource())]
        pub fn set_resource(
            origin: OriginFor<T>,
            id: ResourceId,
            method: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::register_resource(id, method)
        }

        /// Removes a resource ID from the resource mapping.
        ///
        /// After this call, bridge transfers with the associated resource ID will
        /// be rejected.
        ///
        /// # <weight>
        /// - O(1) removal
        /// # </weight>
        #[pallet::call_index(5)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::remove_resource())]
        pub fn remove_resource(origin: OriginFor<T>, id: ResourceId) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::unregister_resource(id)
        }

        /// Enables a chain ID as a source or destination for a bridge transfer.
        ///
        /// # <weight>
        /// - O(1) lookup and insert
        /// # </weight>
        #[pallet::call_index(6)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::whitelist_chain())]
        pub fn whitelist_chain(
            origin: OriginFor<T>,
            id: ChainId,
            fee: T::Balance,
        ) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::whitelist(id, fee)
        }

        /// Adds a new relayer to the relayer set.
        ///
        /// # <weight>
        /// - O(1) lookup and insert
        /// # </weight>
        #[pallet::call_index(7)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::add_relayer())]
        pub fn add_relayer(origin: OriginFor<T>, v: T::AccountId) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::register_relayer(v)
        }

        /// Removes an existing relayer from the set.
        ///
        /// # <weight>
        /// - O(1) lookup and removal
        /// # </weight>
        #[pallet::call_index(8)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::remove_relayer())]
        pub fn remove_relayer(origin: OriginFor<T>, v: T::AccountId) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            Self::unregister_relayer(v)
        }

        /// Sets minimal deposit nonce for chain id
        ///
        /// # <weight>
        /// - O(1) lookup and insert
        /// # </weight>
        #[pallet::call_index(9)]
        #[pallet::weight(T::DbWeight::get().writes(1).ref_time())]
        pub fn set_min_nonce(
            origin: OriginFor<T>,
            chain_id: ChainId,
            min_nonce: DepositNonce,
        ) -> DispatchResultWithPostInfo {
            T::AdminOrigin::ensure_origin(origin)?;
            MinDepositNonce::<T>::insert(chain_id, min_nonce);
            Ok(().into())
        }

        /// Commits a vote in favour of the provided proposal.
        ///
        /// If a proposal with the given nonce and source chain ID does not already exist, it will
        /// be created with an initial vote in favour from the caller.
        ///
        /// # <weight>
        /// - weight of proposed call, regardless of whether execution is performed
        /// # </weight>
        #[pallet::call_index(10)]
        #[pallet::weight({
            (call.get_dispatch_info().weight + <T as pallet::Config>::WeightInfo::acknowledge_proposal(), DispatchClass::Normal)
        })]
        pub fn acknowledge_proposal(
            origin: OriginFor<T>,
            nonce: DepositNonce,
            src_id: ChainId,
            r_id: ResourceId,
            call: Box<<T as Config>::Proposal>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(Self::is_relayer(&who), Error::<T>::MustBeRelayer);
            ensure!(
                Self::chain_whitelisted(src_id),
                Error::<T>::ChainNotWhitelisted
            );
            ensure!(Self::chain_enabled(src_id), Error::<T>::DisabledChain);
            ensure!(
                Self::resource_exists(r_id),
                Error::<T>::ResourceDoesNotExist
            );
            ensure!(
                Self::min_nonce_threshold(src_id, nonce),
                Error::<T>::MinimalNonce
            );

            Self::vote_for(who, nonce, src_id, call)?;
            Ok(Pays::No.into())
        }

        /// Commits a vote against a provided proposal.
        ///
        /// # <weight>
        /// - Fixed, since execution of proposal should not be included
        /// # </weight>
        #[pallet::call_index(11)]
        #[pallet::weight({(<T as pallet::Config>::WeightInfo::reject_proposal(), DispatchClass::Normal)})]
        pub fn reject_proposal(
            origin: OriginFor<T>,
            nonce: DepositNonce,
            src_id: ChainId,
            r_id: ResourceId,
            call: Box<<T as Config>::Proposal>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(Self::is_relayer(&who), Error::<T>::MustBeRelayer);
            ensure!(
                Self::chain_whitelisted(src_id),
                Error::<T>::ChainNotWhitelisted
            );
            ensure!(Self::chain_enabled(src_id), Error::<T>::DisabledChain);
            ensure!(
                Self::resource_exists(r_id),
                Error::<T>::ResourceDoesNotExist
            );

            Self::vote_against(who, nonce, src_id, call)?;
            Ok(Pays::No.into())
        }

        /// Evaluate the state of a proposal given the current vote threshold.
        ///
        /// A proposal with enough votes will be either executed or cancelled, and the status
        /// will be updated accordingly.
        ///
        /// # <weight>
        /// - weight of proposed call, regardless of whether execution is performed
        /// # </weight>
        #[pallet::call_index(12)]
        #[pallet::weight({
            (prop.get_dispatch_info().weight.saturating_add(<T as pallet::Config>::WeightInfo::eval_vote_state()), DispatchClass::Normal)
        })]
        pub fn eval_vote_state(
            origin: OriginFor<T>,
            nonce: DepositNonce,
            src_id: ChainId,
            prop: Box<<T as Config>::Proposal>,
        ) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            Self::try_resolve_proposal(nonce, src_id, prop)
        }

        /// Redistributes accumulated fees between relayers.
        // TODO add relayers count as argument.
        #[pallet::call_index(13)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::redistribute_fees(10))]
        pub fn redistribute_fees(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;
            Self::do_redistribute_fees()
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Allowability transfers for the chain has changed. \[chain_id, enabled\]
        ChainToggled(ChainId, bool),
        /// Vote threshold has changed. \[new_threshold\]
        RelayerThresholdChanged(u32),
        /// Fee for chain has changed. \[chain_id, fee\]
        FeeChanged(ChainId, T::Balance),
        /// Proposal lifetime has changed. \[lifetime\]
        ProposalLifetimeChanged(BlockNumberFor<T>),
        /// Chain now available for transfers. \[chain_id\]
        ChainWhitelisted(ChainId),
        /// Relayer added to set. \[who\]
        RelayerAdded(T::AccountId),
        /// Relayer removed from set. \[who\]
        RelayerRemoved(T::AccountId),
        /// FunglibleTransfer is for relaying fungibles. \[dest_id, nonce, resource_id, amount, recipient\]
        FungibleTransfer(ChainId, DepositNonce, ResourceId, U256, Vec<u8>),
        /// NonFungibleTransfer is for relaying NFTS. \[dest_id, nonce, resource_id, token_id, recipient, metadata\]
        NonFungibleTransfer(ChainId, DepositNonce, ResourceId, Vec<u8>, Vec<u8>, Vec<u8>),
        /// GenericTransfer is for a generic data payload. \[dest_id, nonce, resource_id, metadata\]
        GenericTransfer(ChainId, DepositNonce, ResourceId, Vec<u8>),
        /// Vote submitted in favour of proposal. \[dest_id, nonce, who\]
        VoteFor(ChainId, DepositNonce, T::AccountId),
        /// Vot submitted against proposal. \[dest_id, nonce, who\]
        VoteAgainst(ChainId, DepositNonce, T::AccountId),
        /// Voting successful for a proposal. \[dest_id, nonce\]
        ProposalApproved(ChainId, DepositNonce),
        /// Voting rejected a proposal. \[dest_id, nonce\]
        ProposalRejected(ChainId, DepositNonce),
        /// Execution of call succeeded. \[dest_id, nonce\]
        ProposalSucceeded(ChainId, DepositNonce),
        /// Execution of call failed. \[dest_id, nonce\]
        ProposalFailed(ChainId, DepositNonce),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Relayer threshold not set
        ThresholdNotSet,
        /// Provided chain Id is not valid
        InvalidChainId,
        /// Relayer threshold cannot be 0
        InvalidThreshold,
        /// Fee cannot be less than 0
        InvalidFee,
        /// Proposal lifetime cannot be equal 0
        InvalidProposalLifetime,
        /// Interactions with this chain is not permitted
        ChainNotWhitelisted,
        /// Chain has already been enabled
        ChainAlreadyWhitelisted,
        /// Resource ID provided isn't mapped to anything
        ResourceDoesNotExist,
        /// Relayer already in set
        RelayerAlreadyExists,
        /// Provided accountId is not a relayer
        RelayerInvalid,
        /// Protected operation, must be performed by relayer
        MustBeRelayer,
        /// Relayer has already submitted some vote for this proposal
        RelayerAlreadyVoted,
        /// A proposal with these parameters has already been submitted
        ProposalAlreadyExists,
        /// No proposal with the ID was found
        ProposalDoesNotExist,
        /// Cannot complete proposal, needs more votes
        ProposalNotComplete,
        /// Proposal has either failed or succeeded
        ProposalAlreadyComplete,
        /// Lifetime of proposal has been exceeded
        ProposalExpired,
        /// Bridge transfers to this chain have equal allowability
        AllowabilityEqual,
        /// Bridge transfers to this chain are disabled
        DisabledChain,
        /// Minimal nonce check not passed
        MinimalNonce,
    }

    /// All whitelisted chains and their respective transaction counts.
    #[pallet::storage]
    #[pallet::getter(fn chains)]
    pub(super) type ChainNonces<T: Config> = StorageMap<_, Blake2_128Concat, ChainId, DepositNonce>;

    /// Permission to voting and making transfers.
    #[pallet::storage]
    #[pallet::getter(fn disabled_chains)]
    pub type DisabledChains<T: Config> = StorageMap<_, Blake2_128Concat, ChainId, (), ValueQuery>;

    /// Minimal allowed value for deposit nonce per chain id
    #[pallet::storage]
    #[pallet::getter(fn min_deposit_nonce)]
    pub type MinDepositNonce<T: Config> =
        StorageMap<_, Blake2_128Concat, ChainId, DepositNonce, OptionQuery>;

    #[pallet::type_value]
    pub(super) fn DefaultForRelayerThreshold() -> u32 {
        DEFAULT_RELAYER_THRESHOLD
    }

    /// Number of votes required for a proposal to execute.
    #[pallet::storage]
    #[pallet::getter(fn relayer_threshold)]
    pub(super) type RelayerThreshold<T: Config> =
        StorageValue<_, u32, ValueQuery, DefaultForRelayerThreshold>;

    /// Tracks current relayer set.
    #[pallet::storage]
    #[pallet::getter(fn relayers)]
    pub type Relayers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, bool, ValueQuery>;

    /// Number of relayers in set.
    #[pallet::storage]
    #[pallet::getter(fn relayer_count)]
    pub type RelayerCount<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// All known proposals.
    /// The key is the hash of the call and the deposit ID, to ensure it's unique.
    #[pallet::storage]
    #[pallet::getter(fn votes)]
    pub type Votes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        ChainId,
        Blake2_128Concat,
        (DepositNonce, T::Proposal),
        ProposalVotes<T::AccountId, BlockNumberFor<T>>,
    >;

    /// Utilized by the bridge software to map resource IDs to actual methods.
    #[pallet::storage]
    #[pallet::getter(fn resources)]
    pub type Resources<T: Config> = StorageMap<_, Blake2_128Concat, ResourceId, Vec<u8>>;

    /// Fee to charge for a bridge transfer S->E.
    #[pallet::storage]
    #[pallet::getter(fn fee)]
    pub type Fees<T: Config> = StorageMap<_, Blake2_128Concat, ChainId, T::Balance, ValueQuery>;

    /// Time in blocks for relays voting
    #[pallet::storage]
    #[pallet::getter(fn proposal_lifetime)]
    pub type ProposalLifetime<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub _runtime: PhantomData<T>,
        pub chains: Vec<ChainId>,
        pub fees: Vec<(ChainId, u128)>,
        pub relayers: Vec<T::AccountId>,
        pub threshold: u32,
        pub resources: Vec<(ResourceId, Vec<u8>)>,
        pub proposal_lifetime: BlockNumberFor<T>,
        pub min_nonces: Vec<(ChainId, DepositNonce)>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                chains: vec![],
                fees: vec![],
                relayers: vec![],
                threshold: DEFAULT_RELAYER_THRESHOLD,
                resources: vec![],
                proposal_lifetime: DEFAULT_PROPOSAL_LIFETIME.into(),
                min_nonces: vec![],
                _runtime: PhantomData,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            use core::convert::TryFrom;

            let extra_genesis_builder: fn(&Self) = |config| {
                use eq_primitives::{EqPalletAccountInitializer, PalletAccountInitializer};
                EqPalletAccountInitializer::<T>::initialize(&Pallet::<T>::account_id());
                EqPalletAccountInitializer::<T>::initialize(&Pallet::<T>::fee_account_id());

                // 1. whitelist-chain
                for chain in config.chains.iter() {
                    ChainNonces::<T>::insert(&chain, 0);
                }

                // 2. add_relayer
                for relayer in config.relayers.iter() {
                    Relayers::<T>::insert(&relayer, true);
                    RelayerCount::<T>::mutate(|i| *i += 1);
                }

                // 3. set_threshold
                RelayerThreshold::<T>::put(config.threshold);

                // 4. set_resource
                for (resource_id, method) in config.resources.iter() {
                    Resources::<T>::insert(resource_id, method);
                }

                //5. set fees
                for (chain, fee) in config.fees.iter() {
                    if let Ok(fee) = T::Balance::try_from(fee * 1_000_000_000) {
                        Fees::<T>::insert(chain, fee);
                    } else {
                        panic!("Fee chould fit in `Balance` type");
                    }
                }

                // 6. set proposal lifetime
                ProposalLifetime::<T>::put(config.proposal_lifetime);

                // 7. min_nonces
                for (chain, n) in config.min_nonces.iter() {
                    MinDepositNonce::<T>::insert(chain, n);
                }
            };
            extra_genesis_builder(self);
        }
    }
}

impl<T: Config> Pallet<T> {
    // *** Utility methods ***

    /// Checks if who is a relayer.
    pub fn is_relayer(who: &T::AccountId) -> bool {
        Self::relayers(who)
    }

    /// Provides an AccountId for the pallet.
    /// This is used both as an origin check and deposit/withdrawal account.
    pub fn account_id() -> T::AccountId {
        MODULE_ID.into_account_truncating()
    }

    /// Provides an AccountId for the pallet's fee.
    pub fn fee_account_id() -> T::AccountId {
        FEE_MODULE_ID.into_account_truncating()
    }

    /// Asserts if a resource is registered.
    pub fn resource_exists(id: ResourceId) -> bool {
        return Self::resources(id) != None;
    }

    /// Checks if a chain exists as a whitelisted destination.
    pub fn chain_whitelisted(id: ChainId) -> bool {
        return Self::chains(id) != None;
    }

    /// Asserts if transfers with chain are disabled.
    pub fn chain_enabled(id: ChainId) -> bool {
        return !<DisabledChains<T>>::contains_key(id);
    }

    pub fn min_nonce_threshold(id: ChainId, nonce: DepositNonce) -> bool {
        MinDepositNonce::<T>::get(id)
            .map(|min| nonce >= min)
            .unwrap_or(false)
    }

    /// Increments the deposit nonce for the specified chain ID.
    fn bump_nonce(id: ChainId) -> DepositNonce {
        let nonce = Self::chains(id).unwrap_or_default() + 1;
        ChainNonces::<T>::insert(id, nonce);
        nonce
    }

    /// Returns account ids of all registered relayers.
    fn collect_relayers() -> Vec<T::AccountId> {
        <Relayers<T>>::iter()
            .filter_map(|(k, v)| if v { Some(k) } else { None })
            .collect()
    }

    // *** Admin methods ***

    /// Set a new voting threshold.
    pub fn set_relayer_threshold(threshold: u32) -> DispatchResultWithPostInfo {
        ensure!(threshold > 0, Error::<T>::InvalidThreshold);
        RelayerThreshold::<T>::put(threshold);
        Self::deposit_event(Event::RelayerThresholdChanged(threshold));
        Ok(().into())
    }

    pub fn set_chain_fee(chain_id: ChainId, fee: T::Balance) -> DispatchResultWithPostInfo {
        use sp_runtime::traits::Zero;
        ensure!(fee >= T::Balance::zero(), Error::<T>::InvalidFee);
        Fees::<T>::insert(chain_id, fee);
        Self::deposit_event(Event::FeeChanged(chain_id, fee));
        Ok(().into())
    }

    pub fn set_lifetime(lifetime: BlockNumberFor<T>) -> DispatchResultWithPostInfo {
        use sp_runtime::traits::Zero;
        ensure!(
            lifetime > BlockNumberFor::<T>::zero(),
            Error::<T>::InvalidProposalLifetime
        );
        ProposalLifetime::<T>::put(lifetime);
        Self::deposit_event(Event::ProposalLifetimeChanged(lifetime));
        Ok(().into())
    }

    pub fn toggle_chain_state(chain_id: ChainId, enabled: bool) -> DispatchResultWithPostInfo {
        let actual_state = Self::chain_enabled(chain_id);
        ensure!(actual_state != enabled, Error::<T>::AllowabilityEqual);
        if enabled {
            DisabledChains::<T>::remove(chain_id);
        } else {
            DisabledChains::<T>::insert(chain_id, ());
        }

        Self::deposit_event(Event::ChainToggled(chain_id, enabled));
        Ok(().into())
    }

    /// Register a method for a resource Id, enabling associated transfers.
    pub fn register_resource(id: ResourceId, method: Vec<u8>) -> DispatchResultWithPostInfo {
        Resources::<T>::insert(id, method);
        Ok(().into())
    }

    /// Removes a resource ID, disabling associated transfer.
    pub fn unregister_resource(id: ResourceId) -> DispatchResultWithPostInfo {
        Resources::<T>::remove(id);
        Ok(().into())
    }

    /// Whitelist a chain ID for transfer.
    pub fn whitelist(id: ChainId, fee: T::Balance) -> DispatchResultWithPostInfo {
        // Cannot whitelist this chain
        ensure!(id != T::ChainIdentity::get(), Error::<T>::InvalidChainId);
        // Cannot whitelist with an existing entry
        ensure!(
            !Self::chain_whitelisted(id),
            Error::<T>::ChainAlreadyWhitelisted
        );

        Self::set_chain_fee(id, fee)?;

        ChainNonces::<T>::insert(&id, 0);

        Self::deposit_event(Event::ChainWhitelisted(id));

        Ok(().into())
    }

    /// Adds a new relayer to the set.
    pub fn register_relayer(relayer: T::AccountId) -> DispatchResultWithPostInfo {
        ensure!(
            !Self::is_relayer(&relayer),
            Error::<T>::RelayerAlreadyExists
        );
        <Relayers<T>>::insert(&relayer, true);
        RelayerCount::<T>::mutate(|i| *i += 1);

        Self::deposit_event(Event::RelayerAdded(relayer));
        Ok(().into())
    }

    /// Removes a relayer from the set.
    pub fn unregister_relayer(relayer: T::AccountId) -> DispatchResultWithPostInfo {
        ensure!(Self::is_relayer(&relayer), Error::<T>::RelayerInvalid);
        <Relayers<T>>::remove(&relayer);
        RelayerCount::<T>::mutate(|i| *i -= 1);
        Self::deposit_event(Event::RelayerRemoved(relayer));
        Ok(().into())
    }

    /// Redistributes accumulated fees between relayers.
    fn do_redistribute_fees() -> DispatchResultWithPostInfo {
        use sp_runtime::traits::Zero;

        let basic_asset = <eq_assets::Pallet<T>>::get_main_asset();
        let fee_account = Self::fee_account_id();
        let fee_balance = T::BalanceGetter::get_balance(&fee_account, &basic_asset);
        if fee_balance.is_zero() {
            return Ok(().into());
        }
        if let SignedBalance::Positive(fee_balance_value) = fee_balance {
            let count = RelayerCount::<T>::get();
            if count == 0 {
                return Ok(().into());
            }
            let share = fee_balance_value / count.into();
            if share.is_zero() {
                return Ok(().into());
            }
            let relayers = Self::collect_relayers();
            for relayer in relayers {
                <T as Config>::Currency::transfer(
                    &fee_account,
                    &relayer,
                    share,
                    ExistenceRequirement::AllowDeath,
                )?;
            }
        } else {
            panic!("A fee account can't have negative balance");
        }
        Ok(().into())
    }

    // *** Proposal voting and execution methods ***

    /// Commits a vote for a proposal. If the proposal doesn't exist it will be created.
    fn commit_vote(
        who: T::AccountId,
        nonce: DepositNonce,
        src_id: ChainId,
        prop: Box<T::Proposal>,
        in_favour: bool,
    ) -> DispatchResultWithPostInfo {
        let now = <frame_system::Pallet<T>>::block_number();
        let mut votes = match <Votes<T>>::get(src_id, (nonce, prop.clone())) {
            Some(v) => v,
            None => {
                let mut v = ProposalVotes::default();
                v.expiry = now + ProposalLifetime::<T>::get();
                v
            }
        };

        // Ensure the proposal isn't complete and relayer hasn't already voted
        ensure!(!votes.is_complete(), Error::<T>::ProposalAlreadyComplete);
        ensure!(!votes.is_expired(now), Error::<T>::ProposalExpired);
        ensure!(!votes.has_voted(&who), Error::<T>::RelayerAlreadyVoted);

        if in_favour {
            votes.votes_for.push(who.clone());
            Self::deposit_event(Event::VoteFor(src_id, nonce, who.clone()));
        } else {
            votes.votes_against.push(who.clone());
            Self::deposit_event(Event::VoteAgainst(src_id, nonce, who.clone()));
        }

        <Votes<T>>::insert(src_id, (nonce, prop.clone()), votes.clone());

        Ok(().into())
    }

    /// Attempts to finalize or cancel the proposal if the vote count allows.
    fn try_resolve_proposal(
        nonce: DepositNonce,
        src_id: ChainId,
        prop: Box<T::Proposal>,
    ) -> DispatchResultWithPostInfo {
        if let Some(mut votes) = <Votes<T>>::get(src_id, (nonce, prop.clone())) {
            let now = <system::Pallet<T>>::block_number();
            ensure!(!votes.is_complete(), Error::<T>::ProposalAlreadyComplete);
            ensure!(!votes.is_expired(now), Error::<T>::ProposalExpired);

            let status =
                votes.try_to_complete(RelayerThreshold::<T>::get(), RelayerCount::<T>::get());
            <Votes<T>>::insert(src_id, (nonce, prop.clone()), votes.clone());

            match status {
                ProposalStatus::Approved => Self::finalize_execution(src_id, nonce, prop),
                ProposalStatus::Rejected => Self::cancel_execution(src_id, nonce),
                _ => Ok(().into()),
            }
        } else {
            Err(Error::<T>::ProposalDoesNotExist)?
        }
    }

    /// Commits a vote in favour of the proposal and executes it if the vote threshold is met.
    fn vote_for(
        who: T::AccountId,
        nonce: DepositNonce,
        src_id: ChainId,
        prop: Box<T::Proposal>,
    ) -> DispatchResultWithPostInfo {
        Self::commit_vote(who, nonce, src_id, prop.clone(), true)?;
        Self::try_resolve_proposal(nonce, src_id, prop)
    }

    /// Commits a vote against the proposal and cancels it if more than (relayers.len() - threshold)
    /// votes against exist.
    fn vote_against(
        who: T::AccountId,
        nonce: DepositNonce,
        src_id: ChainId,
        prop: Box<T::Proposal>,
    ) -> DispatchResultWithPostInfo {
        Self::commit_vote(who, nonce, src_id, prop.clone(), false)?;
        Self::try_resolve_proposal(nonce, src_id, prop)
    }

    /// Execute the proposal and signals the result as an event.
    fn finalize_execution(
        src_id: ChainId,
        nonce: DepositNonce,
        call: Box<T::Proposal>,
    ) -> DispatchResultWithPostInfo {
        Self::deposit_event(Event::ProposalApproved(src_id, nonce));
        call.dispatch(frame_system::RawOrigin::Signed(Self::account_id()).into())
            .map(|_| ())
            .map_err(|e| e.error)?;
        Self::deposit_event(Event::ProposalSucceeded(src_id, nonce));
        Ok(().into())
    }

    /// Cancels a proposal.
    fn cancel_execution(src_id: ChainId, nonce: DepositNonce) -> DispatchResultWithPostInfo {
        Self::deposit_event(Event::ProposalRejected(src_id, nonce));
        Ok(().into())
    }

    /// Initiates a transfer of a fungible asset out of the chain. This should be called by another pallet.
    pub fn transfer_fungible(
        dest_id: ChainId,
        resource_id: ResourceId,
        to: Vec<u8>,
        amount: U256,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            Self::chain_whitelisted(dest_id),
            Error::<T>::ChainNotWhitelisted
        );
        ensure!(Self::chain_enabled(dest_id), Error::<T>::DisabledChain);
        let nonce = Self::bump_nonce(dest_id);
        Self::deposit_event(Event::FungibleTransfer(
            dest_id,
            nonce,
            resource_id,
            amount,
            to,
        ));
        Ok(().into())
    }

    /// Initiates a transfer of a nonfungible asset out of the chain. This should be called by another pallet.
    pub fn transfer_nonfungible(
        dest_id: ChainId,
        resource_id: ResourceId,
        token_id: Vec<u8>,
        to: Vec<u8>,
        metadata: Vec<u8>,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            Self::chain_whitelisted(dest_id),
            Error::<T>::ChainNotWhitelisted
        );
        let nonce = Self::bump_nonce(dest_id);
        Self::deposit_event(Event::NonFungibleTransfer(
            dest_id,
            nonce,
            resource_id,
            token_id,
            to,
            metadata,
        ));
        Ok(().into())
    }

    /// Initiates a transfer of generic data out of the chain. This should be called by another pallet.
    pub fn transfer_generic(
        dest_id: ChainId,
        resource_id: ResourceId,
        metadata: Vec<u8>,
    ) -> DispatchResultWithPostInfo {
        ensure!(
            Self::chain_whitelisted(dest_id),
            Error::<T>::ChainNotWhitelisted
        );
        let nonce = Self::bump_nonce(dest_id);
        Self::deposit_event(Event::GenericTransfer(
            dest_id,
            nonce,
            resource_id,
            metadata,
        ));
        Ok(().into())
    }
}

/// Simple ensure origin for the bridge account.
pub struct EnsureBridge<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> EnsureOrigin<T::RuntimeOrigin> for EnsureBridge<T> {
    type Success = T::AccountId;
    fn try_origin(o: T::RuntimeOrigin) -> Result<Self::Success, T::RuntimeOrigin> {
        let bridge_id = MODULE_ID.into_account_truncating();
        o.into().and_then(|o| match o {
            system::RawOrigin::Signed(who) if who == bridge_id => Ok(bridge_id),
            r => Err(T::RuntimeOrigin::from(r)),
        })
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn successful_origin() -> T::Origin {
        T::Origin::from(system::RawOrigin::Signed(
            T::AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes()).unwrap(),
        ))
    }
}
