use std::mem;
use super::{Error, Operation, ImportOperation};
use crate::traits::{HashOf, BlockOf, Block, BlockExecutor, Backend, BaseContext, AsExternalities};

pub struct Importer<C: BaseContext, B: Backend<C>, E> {
	executor: E,
	backend: B,
	pending: Operation<C, B>,
}

impl<C: BaseContext, B, E> Importer<C, B, E> where
	B: Backend<C, Operation=Operation<C, B>>,
	E: BlockExecutor<C>,
{
	pub fn new(backend: B, executor: E) -> Self {
		Self {
			executor, backend,
			pending: Default::default(),
		}
	}

	pub fn backend(&self) -> &B {
		&self.backend
	}

	pub fn import_block(&mut self, block: BlockOf<C>) -> Result<(), Error> {
		let mut state = self.backend
			.state_at(block.parent_hash().ok_or(Error::IsGenesis)?)
			.map_err(|e| Error::Backend(Box::new(e)))?;
		self.executor.execute_block(&block, state.as_externalities())
			.map_err(|e| Error::Executor(Box::new(e)))?;

		let operation = ImportOperation { block, state };
		self.import_raw(operation)
	}

	pub fn import_raw(&mut self, operation: ImportOperation<C, B>) -> Result<(), Error> {
		self.pending.import_block.push(operation);

		Ok(())
	}

	pub fn set_head(&mut self, head: HashOf<C>) -> Result<(), Error> {
		self.pending.set_head = Some(head);

		Ok(())
	}

	pub fn commit(&mut self) -> Result<(), Error> {
		let mut operation = Operation::default();
		mem::swap(&mut operation, &mut self.pending);

		self.backend.commit(operation)
			.map_err(|e| Error::Backend(Box::new(e)))?;

		Ok(())
	}

	pub fn discard(&mut self) -> Result<(), Error> {
		self.pending = Operation::default();

		Ok(())
	}
}
