use std::marker::PhantomData;
use std::{mem, error as stderror};
use crate::traits::{HashOf, BlockOf, Block, Executor, Backend, Context, AsExternalities};

pub struct ImportOperation<B: Backend> {
	pub block: BlockOf<B::Context>,
	pub state: B::State,
}

pub struct Operation<B: Backend> {
	pub import_block: Vec<ImportOperation<B>>,
	pub set_head: Option<HashOf<B::Context>>,
}

impl<B: Backend> Default for Operation<B> {
	fn default() -> Self {
		Self {
			import_block: Vec::new(),
			set_head: None,
		}
	}
}

pub struct Chain<C: Context, B: Backend, E> {
	executor: E,
	backend: B,
	pending: Operation<B>,
	_marker: PhantomData<C>,
}

pub enum Error {
	Backend(Box<stderror::Error>),
	Executor(Box<stderror::Error>),
	/// Block is genesis block and cannot be imported.
	IsGenesis,
}

impl<C: Context, B, E> Chain<C, B, E> where
	B: Backend<Context=C, Operation=Operation<B>>,
	E: Executor<Context=C>,
{
	pub fn new(backend: B, executor: E) -> Self {
		Self {
			executor, backend,
			pending: Default::default(),
			_marker: Default::default(),
		}
	}

	pub fn import_block(&mut self, block: BlockOf<C>) -> Result<(), Error> {
		let mut state = self.backend.state_at(
			block.parent_hash().ok_or(Error::IsGenesis)?
		).map_err(|e| Error::Backend(Box::new(e)))?;
		self.executor.execute_block(&block, state.as_externalities())
			.map_err(|e| Error::Executor(Box::new(e)))?;

		let operation = ImportOperation { block, state };
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
