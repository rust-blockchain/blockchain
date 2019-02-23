use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::{fmt, error as stderror};

use crate::traits::{
	HashOf, BlockOf, ExternalitiesOf, AsExternalities, Context, Backend,
	NullExternalities, StorageExternalities,
};
use crate::importer::Operation;

#[derive(Debug)]
pub enum Error {
	IO,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::IO => "IO failure".fmt(f)?,
		}

		Ok(())
	}
}

impl stderror::Error for Error { }

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
	fn read_storage(&self, key: &[u8]) -> Vec<u8> {
		unimplemented!()
	}

	fn write_storage(&self, key: &[u8], value: &[u8]) {
		unimplemented!()
	}
}

impl AsExternalities<dyn StorageExternalities> for MemoryState {
	fn as_externalities(&mut self) -> &mut (dyn StorageExternalities + 'static) {
		self
	}
}

pub struct MemoryBackend<C: Context> {
	blocks_and_states: HashMap<HashOf<C>, (BlockOf<C>, MemoryState)>,
}

impl<C: Context> Backend<C> for Arc<RwLock<MemoryBackend<C>>> where
	MemoryState: AsExternalities<ExternalitiesOf<C>>
{
	type State = MemoryState;
	type Operation = Operation<C, Self>;
	type Error = Error;

	fn state_at(
		&self,
		_hash: HashOf<C>,
	) -> Result<MemoryState, Error> {
		unimplemented!()
	}

	fn commit(
		&self,
		operation: Operation<C, Self>,
	) -> Result<(), Error> {
		unimplemented!()
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use super::*;
	use crate::traits::*;
	use crate::importer::Importer;

	pub struct DummyBlock(usize);

	impl Block for DummyBlock {
		type Hash = usize;

		fn hash(&self) -> usize { self.0 }
		fn parent_hash(&self) -> Option<usize> { if self.0 == 0 { None } else { Some(self.0 - 1) } }
	}

	pub trait CombinedExternalities: NullExternalities + StorageExternalities { }

	impl<T: NullExternalities + StorageExternalities> CombinedExternalities for T { }

	impl<T: CombinedExternalities + 'static> AsExternalities<dyn CombinedExternalities> for T {
		fn as_externalities(&mut self) -> &mut (dyn CombinedExternalities + 'static) {
			self
		}
	}

	pub struct DummyContext;

	impl Context for DummyContext {
		type Block = DummyBlock;
		type Externalities = dyn CombinedExternalities + 'static;
	}

	pub struct DummyExecutor;

	impl Executor<DummyContext> for Arc<DummyExecutor> {
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
		}));
		let executor: Arc<DummyExecutor> = Arc::new(DummyExecutor);
		let _ = Importer::new(backend, executor);
	}
}
