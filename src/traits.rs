use std::error as stderror;

pub trait Block {
	type Hash;

	fn hash(&self) -> Self::Hash;
	fn parent_hash(&self) -> Option<Self::Hash>;
}

pub type ExternalitiesOf<C> = <C as Context>::Externalities;
pub type BlockOf<C> = <C as Context>::Block;
pub type HashOf<C> = <BlockOf<C> as Block>::Hash;

pub trait Context {
	type Block: Block;
	type Externalities: ?Sized;
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

pub trait Backend<C: Context>: Sized {
	type State: AsExternalities<ExternalitiesOf<C>>;
	type Operation;
	type Error: stderror::Error + 'static;

	fn state_at(
		&self,
		hash: HashOf<C>,
	) -> Result<Self::State, Self::Error>;

	fn commit(
		&self,
		operation: Self::Operation,
	) -> Result<(), Self::Error>;
}

pub trait Executor<C: Context>: Sized {
	type Error: stderror::Error + 'static;

	fn execute_block(
		&self,
		block: &BlockOf<C>,
		state: &mut ExternalitiesOf<C>
	) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;
	use std::error as stderror;
	use std::fmt;
	use std::sync::{Arc, RwLock};

	use crate::importer::Operation;

	pub struct DummyBlock(usize);

	impl Block for DummyBlock {
		type Hash = usize;

		fn hash(&self) -> usize { self.0 }
		fn parent_hash(&self) -> Option<usize> { if self.0 == 0 { None } else { Some(self.0 - 1) } }
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

		fn state_at(
			&self,
			_hash: usize
		) -> Result<DummyState, DummyError> {
			let _ = self.read().expect("backend lock is poisoned");

			Ok(DummyState {
				_backend: self.clone()
			})
		}

		fn commit(
			&self,
			operation: Operation<DummyContext, Self>,
		) -> Result<(), DummyError> {
			let mut this = self.write().expect("backend lock is poisoned");
			for block in operation.import_block {
				this.blocks.insert(block.block.0, block.block);
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

	impl Context for DummyContext {
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

	impl Executor<DummyContext> for Arc<DummyExecutor> {
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
