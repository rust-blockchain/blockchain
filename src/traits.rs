use std::error as stderror;
use std::hash;

pub trait Block: Clone {
	type Hash: Copy + Eq + hash::Hash;

	fn hash(&self) -> &Self::Hash;
	fn parent_hash(&self) -> Option<&Self::Hash>;
}

pub type ExternalitiesOf<C> = <C as BaseContext>::Externalities;
pub type BlockOf<C> = <C as BaseContext>::Block;
pub type HashOf<C> = <BlockOf<C> as Block>::Hash;
pub type ExtrinsicOf<C> = <C as ExtrinsicContext>::Extrinsic;

pub trait BaseContext {
	type Block: Block;
	type Externalities: ?Sized;
}

pub trait ExtrinsicContext: BaseContext {
	type Extrinsic;
}

pub trait AsExternalities<E: ?Sized> {
	fn as_externalities(&mut self) -> &mut E;
}

pub trait NullExternalities { }

pub trait StorageExternalities {
	fn read_storage(&self, key: &[u8]) -> Option<Vec<u8>>;
	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>);
	fn remove_storage(&mut self, key: &[u8]);
}

pub trait Backend<C: BaseContext>: Sized {
	type State: AsExternalities<ExternalitiesOf<C>>;
	type Operation;
	type Error: stderror::Error + 'static;

	fn state_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Option<Self::State>, Self::Error>;

	fn block_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Option<BlockOf<C>>, Self::Error>;

	fn commit(
		&self,
		operation: Self::Operation,
	) -> Result<(), Self::Error>;
}

pub trait BlockExecutor<C: BaseContext>: Sized {
	type Error: stderror::Error + 'static;

	fn execute_block(
		&self,
		block: &BlockOf<C>,
		state: &mut ExternalitiesOf<C>
	) -> Result<(), Self::Error>;
}

pub trait BuilderExecutor<C: ExtrinsicContext>: Sized {
	type Error: stderror::Error + 'static;

	fn initialize_block(
		&self,
		block: &mut BlockOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;

	fn apply_extrinsic(
		&self,
		block: &mut BlockOf<C>,
		extrinsic: ExtrinsicOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;

	fn finalize_block(
		&self,
		block: &mut BlockOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;
	use std::error as stderror;
	use std::fmt;
	use std::sync::{Arc, RwLock};

	use crate::chain::Operation;

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

	pub struct DummyBackendInner {
		blocks: HashMap<usize, DummyBlock>,
		head: usize,
	}

	pub type DummyBackend = RwLock<DummyBackendInner>;

	impl Backend<DummyContext> for Arc<DummyBackend> {
		type State = DummyState;
		type Error = DummyError;
		type Operation = Operation<DummyContext, Self>;

		fn block_at(
			&self,
			hash: &usize
		) -> Result<Option<DummyBlock>, DummyError> {
			let this = self.read().expect("backend lock is poisoned");
			Ok(this.blocks.get(hash).cloned())
		}

		fn state_at(
			&self,
			hash: &usize
		) -> Result<Option<DummyState>, DummyError> {
			let this = self.read().expect("backend lock is poisoned");
			Ok(if this.blocks.contains_key(hash) {
				Some(DummyState {
					_backend: self.clone()
				})
			} else {
				None
			})
		}

		fn commit(
			&self,
			operation: Operation<DummyContext, Self>,
		) -> Result<(), DummyError> {
			let mut this = self.write().expect("backend lock is poisoned");
			for block in operation.import_block {
				this.blocks.insert(*block.block.hash(), block.block);
			}
			if let Some(head) = operation.set_head {
				this.head = head;
			}

			Ok(())
		}
	}

	pub struct DummyState {
		_backend: Arc<DummyBackend>,
	}

	pub trait DummyExternalities {
		fn test_fn(&self) -> usize { 42 }
	}

	impl DummyExternalities for DummyState { }

	impl AsExternalities<dyn DummyExternalities> for DummyState {
		fn as_externalities(&mut self) -> &mut (dyn DummyExternalities + 'static) {
			self
		}
	}

	pub struct DummyContext;

	impl BaseContext for DummyContext {
		type Block = DummyBlock;
		type Externalities = dyn DummyExternalities + 'static;
	}

	#[derive(Debug)]
	pub struct DummyError;

	impl fmt::Display for DummyError {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			"dummy error".fmt(f)
		}
	}

	impl stderror::Error for DummyError { }

	pub struct DummyExecutor;

	impl BlockExecutor<DummyContext> for Arc<DummyExecutor> {
		type Error = DummyError;

		fn execute_block(
			&self,
			_block: &DummyBlock,
			_state: &mut (dyn DummyExternalities + 'static),
		) -> Result<(), DummyError> {
			Ok(())
		}
	}
}
