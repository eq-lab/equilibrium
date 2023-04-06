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

use super::{AccountId, CouncilInstance, TechnicalCommitteeInstance};
use frame_support::traits::EitherOfDiverse;
use frame_system::EnsureRoot;
use pallet_collective::*;

pub type EnsureRootOrAllCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionAtLeast<AccountId, CouncilInstance, 1, 1>,
>;

pub type EnsureRootOrMoreThanHalfCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionMoreThan<AccountId, CouncilInstance, 1, 2>,
>;

pub type EnsureRootOrHalfCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionAtLeast<AccountId, CouncilInstance, 1, 2>,
>;

pub type EnsureRootOrTwoThirdsCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionAtLeast<AccountId, CouncilInstance, 2, 3>,
>;

pub type EnsureRootOrThreeForthsCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionAtLeast<AccountId, CouncilInstance, 3, 4>,
>;

pub type EnsureAtLeastOneOfCouncil = EitherOfDiverse<
    EnsureMember<AccountId, CouncilInstance>,
    EnsureMembers<AccountId, CouncilInstance, 1>,
>;

pub type EnsureRootOrAllTechnicalCommittee = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 1, 1>,
>;

pub type EnsureRootOrMoreThanHalfTechnicalCommittee = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionMoreThan<AccountId, TechnicalCommitteeInstance, 1, 2>,
>;

pub type EnsureRootOrHalfTechnicalCommittee = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 1, 2>,
>;

pub type EnsureRootOrTwoThirdsTechnicalCommittee = EitherOfDiverse<
    EnsureRoot<AccountId>,
    EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 2, 3>,
>;

pub type EnsureAtLeastOneOfTechnicalCommittee = EitherOfDiverse<
    EnsureMember<AccountId, TechnicalCommitteeInstance>,
    EnsureMembers<AccountId, TechnicalCommitteeInstance, 1>,
>;
