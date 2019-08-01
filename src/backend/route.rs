// Copyright 2019 Wei Tang.
// This file is part of Rust Blockchain.

// Copyright 2017-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// This is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// It is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

use crate::Block;
use crate::backend::ChainQuery;

/// A tree-route from one block to another in the chain.
///
/// All blocks prior to the pivot in the deque is the reverse-order unique ancestry
/// of the first block, the block at the pivot index is the common ancestor,
/// and all blocks after the pivot is the ancestry of the second block, in
/// order.
///
/// The ancestry sets will include the given blocks, and thus the tree-route is
/// never empty.
///
/// ```text
/// Tree route from R1 to E2. Retracted is [R1, R2, R3], Common is C, enacted [E1, E2]
///   <- R3 <- R2 <- R1
///  /
/// C
///  \-> E1 -> E2
/// ```
///
/// ```text
/// Tree route from C to E2. Retracted empty. Common is C, enacted [E1, E2]
/// C -> E1 -> E2
/// ```
pub struct TreeRoute<B: Block> {
	route: Vec<B::Identifier>,
	pivot: usize,
}

impl<B: Block> TreeRoute<B> {
	/// Get a slice of all retracted blocks in reverse order (towards common ancestor)
	pub fn retracted(&self) -> &[B::Identifier] {
		&self.route[..self.pivot]
	}

	/// Get the common ancestor block. This might be one of the two blocks of the
	/// route.
	pub fn common_block(&self) -> &B::Identifier {
		self.route.get(self.pivot).expect("tree-routes are computed between blocks; \
			which are included in the route; \
			thus it is never empty; qed")
	}

	/// Get a slice of enacted blocks (descendents of the common ancestor)
	pub fn enacted(&self) -> &[B::Identifier] {
		&self.route[self.pivot + 1 ..]
	}
}

/// Compute a tree-route between two blocks. See tree-route docs for more details.
pub fn tree_route<Ba: ChainQuery>(
	backend: &Ba,
	from_id: &<Ba::Block as Block>::Identifier,
	to_id: &<Ba::Block as Block>::Identifier,
) -> Result<TreeRoute<Ba::Block>, Ba::Error> {
	let mut from = backend.block_at(from_id)?;
	let mut to = backend.block_at(to_id)?;

	let mut from_branch = Vec::new();
	let mut to_branch = Vec::new();

	{
		let mut from_depth = backend.depth_at(from_id)?;
		let mut to_depth = backend.depth_at(to_id)?;

		while to_depth > from_depth {
			let to_parent_id = match to.parent_id() {
				Some(parent_id) => parent_id,
				None => {
					assert!(to_depth == 0, "When parent_id is None, depth should be 0");
					break;
				}
			};

			to_branch.push(to.id());
			to = backend.block_at(&to_parent_id)?;
			to_depth = backend.depth_at(&to_parent_id)?;
		}

		while from_depth > to_depth {
			let from_parent_id = match from.parent_id() {
				Some(parent_id) => parent_id,
				None => {
					assert!(to_depth == 0, "When parent_id is None, depth should be 0");
					break;
				}
			};

			from_branch.push(from.id());
			from = backend.block_at(&from_parent_id)?;
			from_depth = backend.depth_at(&from_parent_id)?;
		}
	}

	while from.id() != to.id() {
		let to_parent_id = match to.parent_id() {
			Some(parent_id) => parent_id,
			None => {
				panic!("During backend import, all blocks are checked to have parent; this branch is when common parent does not exist; qed");
			}
		};

		let from_parent_id = match from.parent_id() {
			Some(parent_id) => parent_id,
			None => {
				panic!("During backend import, all blocks are checked to have parent; this branch is when common parent does not exist; qed");
			}
		};

		to_branch.push(to.id());
		to = backend.block_at(&to_parent_id)?;

		from_branch.push(from.id());
		from = backend.block_at(&from_parent_id)?;
	}

	// add the pivot block. and append the reversed to-branch (note that it's reverse order originalls)
	let pivot = from_branch.len();
	from_branch.push(to.id());
	from_branch.extend(to_branch.into_iter().rev());

	Ok(TreeRoute {
		route: from_branch,
		pivot,
	})
}
