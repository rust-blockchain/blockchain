use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::{fmt, error as stderror};

use crate::traits::{
	HashOf, BlockOf, ExternalitiesOf, AsExternalities, BaseContext, Backend,
	NullExternalities, StorageExternalities, Block,
};
use crate::chain::Operation;

#[derive(Debug)]
pub enum Error {
	IO,
	InvalidOperation,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::IO => "IO failure".fmt(f)?,
			Error::InvalidOperation => "The operation provided is invalid".fmt(f)?,
		}

		Ok(())
	}
}

impl stderror::Error for Error { }

#[derive(Clone)]
pub struct MemoryState {
	storage: HashMap<Vec<u8>, Vec<u8>>,
}

impl NullExternalities for MemoryState { }

impl AsExternalities<dyn NullExternalities> for MemoryState {
	fn as_externalities(&mut self) -> &mut (dyn NullExternalities + 'static) {
		self
	}
}

impl StorageExternalities for MemoryState {
	fn read_storage(&self, key: &[u8]) -> Option<Vec<u8>> {
		self.storage.get(key).map(|value| value.to_vec())
	}

	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>) {
		self.storage.insert(key, value);
	}

	fn remove_storage(&mut self, key: &[u8]) {
		self.storage.remove(key);
	}
}

impl AsExternalities<dyn StorageExternalities> for MemoryState {
	fn as_externalities(&mut self) -> &mut (dyn StorageExternalities + 'static) {
		self
	}
}

pub struct MemoryBackend<C: BaseContext> {
	blocks_and_states: HashMap<HashOf<C>, (BlockOf<C>, MemoryState)>,
	head: HashOf<C>,
}

impl<C: BaseContext> MemoryBackend<C> where {
	pub fn with_genesis(block: BlockOf<C>, genesis_storage: HashMap<Vec<u8>, Vec<u8>>) -> Self {
		assert!(block.parent_hash().is_none(), "with_genesis must be provided with a genesis block");

		let genesis_hash = *block.hash();
		let genesis_state = MemoryState {
			storage: genesis_storage,
		};
		let mut blocks_and_states = HashMap::new();
		blocks_and_states.insert(*block.hash(), (block, genesis_state));

		Self {
			blocks_and_states,
			head: genesis_hash,
		}
	}
}

impl<C: BaseContext> Backend<C> for Arc<RwLock<MemoryBackend<C>>> where
	MemoryState: AsExternalities<ExternalitiesOf<C>>
{
	type State = MemoryState;
	type Operation = Operation<C, Self>;
	type Error = Error;

	fn block_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Option<BlockOf<C>>, Error> {
		let this = self.read().expect("backend lock is poisoned");

		Ok(this.blocks_and_states.get(hash)
		   .map(|(block, _)| block.clone()))
	}

	fn state_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Option<MemoryState>, Error> {
		let this = self.read().expect("backend lock is poisoned");

		Ok(this.blocks_and_states.get(hash)
		   .map(|(_, state)| state.clone()))
	}

	fn commit(
		&self,
		operation: Operation<C, Self>,
	) -> Result<(), Error> {
		let mut this = self.write().expect("backend lock is poisoned");

		let importing_hashes = operation.import_block
			.iter()
			.map(|op| *op.block.hash())
			.collect::<HashSet<_>>();

		// Do precheck to make sure the import operation is valid.
		for op in &operation.import_block {
			let parent_contains_in_backend_or_importing = op.block.parent_hash()
				.map(|parent_hash| {
					this.blocks_and_states.contains_key(parent_hash) ||
						importing_hashes.contains(parent_hash)
				})
				.unwrap_or(true);

			if !parent_contains_in_backend_or_importing {
				return Err(Error::InvalidOperation);
			}
		}

		// Do precheck to make sure the head going to set exists.
		if let Some(new_head) = &operation.set_head {
			let head_exists = this.blocks_and_states.contains_key(new_head) ||
				importing_hashes.contains(new_head);

			if !head_exists {
				return Err(Error::InvalidOperation);
			}
		}

		for op in operation.import_block {
			this.blocks_and_states.insert(*op.block.hash(), (op.block, op.state));
		}

		if let Some(new_head) = operation.set_head {
			this.head = new_head;
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use super::*;
	use crate::traits::*;
	use crate::chain::Importer;

	#[derive(Clone)]
	pub struct DummyBlock {
		hash: usize,
		parent_hash: usize,
	}

	impl Block for DummyBlock {
		type Hash = usize;

		fn hash(&self) -> &usize { &self.hash }
		fn parent_hash(&self) -> Option<&usize> { if self.parent_hash == 0 { None } else { Some(&self.parent_hash) } }
	}

	pub trait CombinedExternalities: NullExternalities + StorageExternalities { }

	impl<T: NullExternalities + StorageExternalities> CombinedExternalities for T { }

	impl<T: CombinedExternalities + 'static> AsExternalities<dyn CombinedExternalities> for T {
		fn as_externalities(&mut self) -> &mut (dyn CombinedExternalities + 'static) {
			self
		}
	}

	pub struct DummyContext;

	impl BaseContext for DummyContext {
		type Block = DummyBlock;
		type Externalities = dyn CombinedExternalities + 'static;
	}

	pub struct DummyExecutor;

	impl BlockExecutor<DummyContext> for Arc<DummyExecutor> {
		type Error = Error;

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
		let backend: Arc<RwLock<MemoryBackend<DummyContext>>> = Arc::new(RwLock::new(MemoryBackend {
			blocks_and_states: Default::default(),
			head: Default::default(),
		}));
		let executor: Arc<DummyExecutor> = Arc::new(DummyExecutor);
		let _ = Importer::new(backend, executor);
	}
}
