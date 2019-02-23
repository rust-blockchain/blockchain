use std::collections::HashMap;
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
	NotExist,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::IO => "IO failure".fmt(f)?,
			Error::InvalidOperation => "The operation provided is invalid".fmt(f)?,
			Error::NotExist => "Block does not exist".fmt(f)?,
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

impl AsExternalities<dyn StorageExternalities> for MemoryState {
	fn as_externalities(&mut self) -> &mut (dyn StorageExternalities + 'static) {
		self
	}
}

pub struct MemoryBackendInner<C: BaseContext> {
	blocks_and_states: HashMap<HashOf<C>, (BlockOf<C>, MemoryState, usize)>,
	head: HashOf<C>,
}

impl<C: BaseContext> MemoryBackendInner<C> where {
	fn head(&self) -> HashOf<C> {
		self.head
	}

	fn contains(
		&self,
		hash: &HashOf<C>
	) -> Result<bool, Error> {
		Ok(self.blocks_and_states.contains_key(hash))
	}

	fn depth_at(
		&self,
		hash: &HashOf<C>
	) -> Result<usize, Error> {
		self.blocks_and_states.get(hash)
		   .map(|(_, _, depth)| *depth)
		   .ok_or(Error::NotExist)
	}

	fn block_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<BlockOf<C>, Error> {
		self.blocks_and_states.get(hash)
			.map(|(block, _, _)| block.clone())
			.ok_or(Error::NotExist)
	}

	fn state_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<MemoryState, Error> {
		self.blocks_and_states.get(hash)
			.map(|(_, state, _)| state.clone())
			.ok_or(Error::NotExist)
	}
}

pub struct MemoryBackend<C: BaseContext>(Arc<RwLock<MemoryBackendInner<C>>>);

impl<C: BaseContext> Clone for MemoryBackend<C> where
	MemoryState: AsExternalities<ExternalitiesOf<C>>
{
	fn clone(&self) -> Self {
		MemoryBackend(self.0.clone())
	}
}

impl<C: BaseContext> MemoryBackend<C> where
	MemoryState: AsExternalities<ExternalitiesOf<C>>
{
	pub fn with_genesis(block: BlockOf<C>, genesis_storage: HashMap<Vec<u8>, Vec<u8>>) -> Self {
		assert!(block.parent_hash().is_none(), "with_genesis must be provided with a genesis block");

		let genesis_hash = *block.hash();
		let genesis_state = MemoryState {
			storage: genesis_storage,
		};
		let mut blocks_and_states = HashMap::new();
		blocks_and_states.insert(*block.hash(), (block, genesis_state, 0));

		let inner = MemoryBackendInner {
			blocks_and_states,
			head: genesis_hash,
		};

		MemoryBackend(Arc::new(RwLock::new(inner)))
	}
}

impl<C: BaseContext> Backend<C> for MemoryBackend<C> where
	MemoryState: AsExternalities<ExternalitiesOf<C>>
{
	type State = MemoryState;
	type Operation = Operation<C, Self>;
	type Error = Error;

	fn head(&self) -> HashOf<C> {
		self.0.read().expect("backend lock is poisoned")
			.head()
	}

	fn contains(
		&self,
		hash: &HashOf<C>
	) -> Result<bool, Error> {
		self.0.read().expect("backend lock is poisoned")
			.contains(hash)
	}

	fn depth_at(
		&self,
		hash: &HashOf<C>
	) -> Result<usize, Error> {
		self.0.read().expect("backend lock is poisoned")
			.depth_at(hash)
	}

	fn block_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<BlockOf<C>, Error> {
		self.0.read().expect("backend lock is poisoned")
			.block_at(hash)
	}

	fn state_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<MemoryState, Error> {
		self.0.read().expect("backend lock is poisoned")
			.state_at(hash)
	}

	fn commit(
		&self,
		operation: Operation<C, Self>,
	) -> Result<(), Error> {
		let mut this = self.0.write().expect("backend lock is poisoned");

		let mut importing = HashMap::new();
		let mut verifying = operation.import_block;

		// Do precheck to make sure the import operation is valid.
		loop {
			let mut progress = false;
			let mut next_verifying = Vec::new();

			for op in verifying {
				let depth = match op.block.parent_hash() {
					Some(parent_hash) => {
						if this.contains(parent_hash)? {
							Some(this.depth_at(parent_hash)?)
						} else if importing.contains_key(parent_hash) {
							importing.get(parent_hash)
								.map(|(_, _, depth)| *depth)
						} else {
							None
						}
					},
					None => Some(0),
				};

				if let Some(depth) = depth {
					progress = true;
					importing.insert(*op.block.hash(), (op.block, op.state, depth));
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
			let head_exists = this.blocks_and_states.contains_key(new_head) ||
				importing.contains_key(new_head);

			if !head_exists {
				return Err(Error::InvalidOperation);
			}
		}

		this.blocks_and_states.extend(importing);

		if let Some(new_head) = operation.set_head {
			this.head = new_head;
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
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

	#[allow(dead_code)]
	pub struct DummyContext;

	impl BaseContext for DummyContext {
		type Block = DummyBlock;
		type Externalities = dyn CombinedExternalities + 'static;
	}

	pub struct DummyExecutor;

	impl BlockExecutor<DummyContext> for DummyExecutor {
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
		let backend = MemoryBackend::with_genesis(
			DummyBlock {
				hash: 1,
				parent_hash: 0,
			},
			Default::default()
		);
		let executor = DummyExecutor;
		let _ = Importer::new(backend, executor);
	}
}
