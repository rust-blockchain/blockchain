use std::marker::PhantomData;
use std::{mem, fmt, error as stderror};
use crate::traits::{HashOf, BlockOf, Block, BlockExecutor, Backend, BaseContext, AsExternalities};

pub struct ImportOperation<C: BaseContext, B: Backend<C>> {
	pub block: BlockOf<C>,
	pub state: B::State,
}

pub struct Operation<C: BaseContext, B: Backend<C>> {
	pub import_block: Vec<ImportOperation<C, B>>,
	pub set_head: Option<HashOf<C>>,
}

impl<C: BaseContext, B> Default for Operation<C, B> where
	B: Backend<C>
{
	fn default() -> Self {
		Self {
			import_block: Vec::new(),
			set_head: None,
		}
	}
}

pub struct Importer<C: BaseContext, B: Backend<C>, E> {
	executor: E,
	backend: B,
	pending: Operation<C, B>,
	_marker: PhantomData<C>,
}

#[derive(Debug)]
pub enum Error {
	Backend(Box<stderror::Error>),
	Executor(Box<stderror::Error>),
	/// Block is genesis block and cannot be imported.
	IsGenesis,
	/// Parent is not in the backend so block cannot be imported.
	ParentNotFound,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::Backend(_) => "Backend failure".fmt(f)?,
			Error::Executor(_) => "Executor failure".fmt(f)?,
			Error::IsGenesis => "Block is genesis block and cannot be imported".fmt(f)?,
			Error::ParentNotFound => "Parent block cannot be found".fmt(f)?,
		}

		Ok(())
	}
}

impl stderror::Error for Error {
	fn source(&self) -> Option<&(dyn stderror::Error + 'static)> {
		match self {
			Error::Backend(e) => Some(e.as_ref()),
			Error::Executor(e) => Some(e.as_ref()),
			Error::IsGenesis | Error::ParentNotFound => None,
		}
	}
}

impl<C: BaseContext, B, E> Importer<C, B, E> where
	B: Backend<C, Operation=Operation<C, B>>,
	E: BlockExecutor<C>,
{
	pub fn new(backend: B, executor: E) -> Self {
		Self {
			executor, backend,
			pending: Default::default(),
			_marker: Default::default(),
		}
	}

	pub fn import_block(&mut self, block: BlockOf<C>) -> Result<(), Error> {
		let mut state = self.backend
			.state_at(block.parent_hash().ok_or(Error::IsGenesis)?)
			.map_err(|e| Error::Backend(Box::new(e)))?
			.ok_or(Error::ParentNotFound)?;
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
