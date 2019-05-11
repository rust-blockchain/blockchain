use std::collections::HashMap;
use std::{fmt, error as stderror};

use crate::traits::{
	AsExternalities, Backend, NullExternalities,
	StorageExternalities, Block, Auxiliary, Operation,
	ChainQuery,
};
use super::tree_route;

/// A backend type that stores all information in memory.
pub trait MemoryLikeBackend {
	/// Block type.
	type Block;
	/// State type.
	type State;

	/// Create a new memory backend from a genesis block.
	fn new_with_genesis(block: Self::Block, genesis_state: Self::State) -> Self;
}

#[derive(Debug)]
pub enum Error {
	IO,
	InvalidOperation,
	ImportingGenesis,
	NotExist,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::IO => "IO failure".fmt(f)?,
			Error::InvalidOperation => "The operation provided is invalid".fmt(f)?,
			Error::NotExist => "Block does not exist".fmt(f)?,
			Error::ImportingGenesis => "Trying to import another genesis".fmt(f)?,
		}

		Ok(())
	}
}

impl stderror::Error for Error { }

/// State stored in memory.
#[derive(Clone, Default)]
pub struct KeyValueMemoryState {
	storage: HashMap<Vec<u8>, Vec<u8>>,
}

impl AsRef<HashMap<Vec<u8>, Vec<u8>>> for KeyValueMemoryState {
	fn as_ref(&self) -> &HashMap<Vec<u8>, Vec<u8>> {
		&self.storage
	}
}

impl AsMut<HashMap<Vec<u8>, Vec<u8>>> for KeyValueMemoryState {
	fn as_mut(&mut self) -> &mut HashMap<Vec<u8>, Vec<u8>> {
		&mut self.storage
	}
}

impl NullExternalities for KeyValueMemoryState { }

impl AsExternalities<dyn NullExternalities> for KeyValueMemoryState {
	fn as_externalities(&mut self) -> &mut (dyn NullExternalities + 'static) {
		self
	}
}

impl StorageExternalities for KeyValueMemoryState {
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Box<std::error::Error>> {
		Ok(self.storage.get(key).map(|value| value.to_vec()))
	}

	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>) {
		self.storage.insert(key, value);
	}

	fn remove_storage(&mut self, key: &[u8]) {
		self.storage.remove(key);
	}
}

impl AsExternalities<dyn StorageExternalities> for KeyValueMemoryState {
	fn as_externalities(&mut self) -> &mut (dyn StorageExternalities + 'static) {
		self
	}
}

struct BlockData<B: Block, S> {
	block: B,
	state: S,
	depth: usize,
	children: Vec<B::Identifier>,
	is_canon: bool,
}

/// Memory backend.
pub struct MemoryBackend<B: Block, A: Auxiliary<B>, S> {
	blocks_and_states: HashMap<B::Identifier, BlockData<B, S>>,
	head: B::Identifier,
	genesis: B::Identifier,
	canon_depth_mappings: HashMap<usize, B::Identifier>,
	auxiliaries: HashMap<A::Key, A>,
}

impl<B: Block, A: Auxiliary<B>, S: Clone> Backend for MemoryBackend<B, A, S> {
	type Block = B;
	type State = S;
	type Auxiliary = A;
	type Error = Error;

	fn commit(
		&mut self,
		operation: Operation<B, Self::State, A>,
	) -> Result<(), Error> {
		let mut parent_ides = HashMap::new();
		let mut importing: HashMap<B::Identifier, BlockData<B, S>> = HashMap::new();
		let mut verifying = operation.import_block;

		// Do precheck to make sure the import operation is valid.
		loop {
			let mut progress = false;
			let mut next_verifying = Vec::new();

			for op in verifying {
				let parent_depth = match op.block.parent_id() {
					Some(parent_id) => {
						if self.contains(&parent_id)? {
							Some(self.depth_at(&parent_id)?)
						} else if importing.contains_key(&parent_id) {
							importing.get(&parent_id)
								.map(|data| data.depth)
						} else {
							None
						}
					},
					None => return Err(Error::ImportingGenesis),
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
				return Err(Error::InvalidOperation);
			}

			verifying = next_verifying;
		}

		// Do precheck to make sure the head going to set exists.
		if let Some(new_head) = &operation.set_head {
			let head_exists = self.contains(new_head)? ||
				importing.contains_key(new_head);

			if !head_exists {
				return Err(Error::InvalidOperation);
			}
		}

		// Do precheck to make sure auxiliary is valid.
		for aux in &operation.insert_auxiliaries {
			for id in aux.associated() {
				if !(self.contains(&id)? || importing.contains_key(&id)) {
					return Err(Error::InvalidOperation);
				}
			}
		}

		self.blocks_and_states.extend(importing);

		// Fix children at ides.
		for (id, parent_id) in parent_ides {
			self.blocks_and_states.get_mut(&parent_id)
				.expect("Parent id are checked to exist or has been just imported; qed")
				.children.push(id);
		}

		if let Some(new_head) = operation.set_head {
			let route = tree_route(self, &self.head, &new_head)
				.expect("Blocks are checked to exist or importing; qed");

			for id in route.retracted() {
				let mut block = self.blocks_and_states.get_mut(id)
					.expect("Block is fetched from tree_route; it must exist; qed");
				block.is_canon = false;
				self.canon_depth_mappings.remove(&block.depth);
			}

			for id in route.enacted() {
				let mut block = self.blocks_and_states.get_mut(id)
					.expect("Block is fetched from tree_route; it must exist; qed");
				block.is_canon = true;
				self.canon_depth_mappings.insert(block.depth, *id);
			}

			self.head = new_head;
		}

		for aux_key in operation.remove_auxiliaries {
			self.auxiliaries.remove(&aux_key);
		}

		for aux in operation.insert_auxiliaries {
			self.auxiliaries.insert(aux.key(), aux);
		}

		Ok(())
	}
}

impl<B: Block, A: Auxiliary<B>, S: Clone> ChainQuery for MemoryBackend<B, A, S> {
	fn head(&self) -> B::Identifier {
		self.head
	}

	fn genesis(&self) -> B::Identifier {
		self.genesis
	}

	fn contains(
		&self,
		id: &B::Identifier
	) -> Result<bool, Error> {
		Ok(self.blocks_and_states.contains_key(id))
	}

	fn is_canon(
		&self,
		id: &B::Identifier
	) -> Result<bool, Error> {
		self.blocks_and_states.get(id)
			.map(|data| data.is_canon)
			.ok_or(Error::NotExist)
	}

	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<B::Identifier>, Error> {
		Ok(self.canon_depth_mappings.get(&depth)
		   .map(|h| h.clone()))
	}

	fn auxiliary(
		&self,
		key: &A::Key
	) -> Result<Option<A>, Error> {
		Ok(self.auxiliaries.get(key).map(|v| v.clone()))
	}

	fn children_at(
		&self,
		id: &B::Identifier,
	) -> Result<Vec<B::Identifier>, Error> {
		self.blocks_and_states.get(id)
			.map(|data| data.children.clone())
			.ok_or(Error::NotExist)
	}

	fn depth_at(
		&self,
		id: &B::Identifier
	) -> Result<usize, Error> {
		self.blocks_and_states.get(id)
		   .map(|data| data.depth)
		   .ok_or(Error::NotExist)
	}

	fn block_at(
		&self,
		id: &B::Identifier,
	) -> Result<B, Error> {
		self.blocks_and_states.get(id)
			.map(|data| data.block.clone())
			.ok_or(Error::NotExist)
	}

	fn state_at(
		&self,
		id: &B::Identifier,
	) -> Result<Self::State, Error> {
		self.blocks_and_states.get(id)
			.map(|data| data.state.clone())
			.ok_or(Error::NotExist)
	}
}

impl<B: Block, A: Auxiliary<B>, S> MemoryLikeBackend for MemoryBackend<B, A, S> {
	type Block = B;
	type State = S;

	fn new_with_genesis(block: B, genesis_state: S) -> Self {
		assert!(block.parent_id().is_none(), "with_genesis must be provided with a genesis block");

		let genesis_id = block.id();
		let mut blocks_and_states = HashMap::new();
		blocks_and_states.insert(
			block.id(),
			BlockData {
				block,
				state: genesis_state,
				depth: 0,
				children: Vec::new(),
				is_canon: true,
			}
		);
		let mut canon_depth_mappings = HashMap::new();
		canon_depth_mappings.insert(0, genesis_id);

		MemoryBackend {
			blocks_and_states,
			canon_depth_mappings,
			auxiliaries: Default::default(),
			genesis: genesis_id,
			head: genesis_id,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::traits::*;
	use crate::chain::SharedBackend;

	#[derive(Clone)]
	pub struct DummyBlock {
		id: usize,
		parent_id: usize,
	}

	impl Block for DummyBlock {
		type Identifier = usize;

		fn id(&self) -> usize { self.id }
		fn parent_id(&self) -> Option<usize> { if self.parent_id == 0 { None } else { Some(self.parent_id) } }
	}

	pub trait CombinedExternalities: NullExternalities + StorageExternalities { }

	impl<T: NullExternalities + StorageExternalities> CombinedExternalities for T { }

	impl<T: CombinedExternalities + 'static> AsExternalities<dyn CombinedExternalities> for T {
		fn as_externalities(&mut self) -> &mut (dyn CombinedExternalities + 'static) {
			self
		}
	}

	pub struct DummyExecutor;

	impl BlockExecutor for DummyExecutor {
		type Error = Error;
		type Block = DummyBlock;
		type Externalities = dyn CombinedExternalities + 'static;

		fn execute_block(
			&self,
			_block: &DummyBlock,
			_state: &mut (dyn CombinedExternalities + 'static),
		) -> Result<(), Error> {
			Ok(())
		}
	}

	#[test]
	fn all_traits_for_importer_are_satisfied() {
		let backend = MemoryBackend::<_, (), KeyValueMemoryState>::new_with_genesis(
			DummyBlock {
				id: 1,
				parent_id: 0,
			},
			Default::default()
		);
		let executor = DummyExecutor;
		let shared = SharedBackend::new(backend);
		let _ = shared.begin_import(&executor);
	}
}
