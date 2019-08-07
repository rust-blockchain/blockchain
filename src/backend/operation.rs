use std::collections::HashMap;
use crate::{Block, Auxiliary};
use crate::backend::{tree_route, Store, ChainQuery, ChainSettlement, OperationError};

/// Representing raw block data.
pub struct BlockData<B: Block, S> {
	/// Block of the data.
	pub block: B,
	/// Block state.
	pub state: S,
	/// Depth.
	pub depth: usize,
	/// Children of the current block.
	pub children: Vec<B::Identifier>,
	/// Whether the block is on the canonical chain.
	pub is_canon: bool,
}

/// Import operation.
pub struct ImportOperation<B, S> {
	/// Block to be imported.
	pub block: B,
	/// Associated state of the block.
	pub state: S,
}

/// Operation for a backend.
pub struct Operation<B: Block, S, A: Auxiliary<B>> {
	/// Import operation.
	pub import_block: Vec<ImportOperation<B, S>>,
	/// Set head operation.
	pub set_head: Option<B::Identifier>,
	/// Auxiliaries insertion operation.
	pub insert_auxiliaries: Vec<A>,
	/// Auxiliaries removal operation.
	pub remove_auxiliaries: Vec<A::Key>,
}

impl<B: Block, S, A: Auxiliary<B>> Default for Operation<B, S, A> {
	fn default() -> Self {
		Self {
			import_block: Vec::new(),
			set_head: None,
			insert_auxiliaries: Vec::new(),
			remove_auxiliaries: Vec::new(),
		}
	}
}

impl<B: Block, S, A: Auxiliary<B>> Operation<B, S, A> {
	/// Settle the current operation.
	pub fn settle<Ba>(self, backend: &mut Ba) -> Result<(), Ba::Error> where
		Ba: ChainQuery + ChainSettlement + Store<Block=B, State=S, Auxiliary=A>,
		Ba::Error: OperationError,
	{
		let mut parent_ides = HashMap::new();
		let mut importing: HashMap<<Ba::Block as Block>::Identifier, BlockData<Ba::Block, Ba::State>> = HashMap::new();
		let mut verifying = self.import_block;

		// Do precheck to make sure the import self is valid.
		loop {
			let mut progress = false;
			let mut next_verifying = Vec::new();

			for op in verifying {
				let parent_depth = match op.block.parent_id() {
					Some(parent_id) => {
						if backend.contains(&parent_id)? {
							Some(backend.depth_at(&parent_id)?)
						} else if importing.contains_key(&parent_id) {
							importing.get(&parent_id)
								.map(|data| data.depth)
						} else {
							None
						}
					},
					None => return Err(Ba::Error::block_is_genesis()),
				};
				let depth = parent_depth.map(|d| d + 1);

				if let Some(depth) = depth {
					progress = true;
					if let Some(parent_id) = op.block.parent_id() {
						parent_ides.insert(op.block.id(), parent_id);
					}
					importing.insert(op.block.id(), BlockData {
						block: op.block,
						state: op.state,
						depth,
						children: Vec::new(),
						is_canon: false,
					});
				} else {
					next_verifying.push(op)
				}
			}

			if next_verifying.len() == 0 {
				break;
			}

			if !progress {
				return Err(Ba::Error::invalid_operation());
			}

			verifying = next_verifying;
		}

		// Do precheck to make sure the head going to set exists.
		if let Some(new_head) = &self.set_head {
			let head_exists = backend.contains(new_head)? ||
				importing.contains_key(new_head);

			if !head_exists {
				return Err(Ba::Error::invalid_operation());
			}
		}

		// Do precheck to make sure auxiliary is valid.
		for aux in &self.insert_auxiliaries {
			for id in aux.associated() {
				if !(backend.contains(&id)? || importing.contains_key(&id)) {
					return Err(Ba::Error::invalid_operation());
				}
			}
		}

		for (id, data) in importing {
			backend.insert_block(
				id, data.block, data.state, data.depth, data.children, data.is_canon
			);
		}

		// Fix children at ides.
		for (id, parent_id) in parent_ides {
			backend.push_child(parent_id, id);
		}

		if let Some(new_head) = self.set_head {
			let route = tree_route(backend, &backend.head(), &new_head)
				.expect("Blocks are checked to exist or importing; qed");

			for id in route.retracted() {
				backend.set_canon(id.clone(), false);
				let depth = backend.depth_at(id)
					.expect("Block is fetched from tree_route; it must exist; qed");
				backend.remove_canon_depth_mapping(&depth);
			}

			for id in route.enacted() {
				backend.set_canon(id.clone(), true);
				let depth = backend.depth_at(id)
					.expect("Block is fetched from tree_route; it must exist; qed");
				backend.insert_canon_depth_mapping(depth, id.clone());
			}

			backend.set_head(new_head);
		}

		for aux_key in self.remove_auxiliaries {
			backend.remove_auxiliary(&aux_key);
		}

		for aux in self.insert_auxiliaries {
			backend.insert_auxiliary(aux.key(), aux);
		}

		Ok(())
	}
}
