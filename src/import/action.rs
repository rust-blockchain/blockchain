use std::sync::MutexGuard;
use super::Error;
use crate::backend::SharedCommittable;
use crate::traits::{Operation, ImportOperation, Block, BlockExecutor, Backend, AsExternalities, Auxiliary, ChainQuery};

/// Block importer.
pub struct ImportAction<'a, 'executor, E: BlockExecutor, Ba> where
	Ba: Backend<Block=E::Block> + ?Sized,
	Ba::Auxiliary: Auxiliary<E::Block>
{
	executor: &'executor E,
	backend: &'a Ba,
	pending: Operation<E::Block, Ba::State, Ba::Auxiliary>,
	_guard: MutexGuard<'a, ()>,
}

impl<'a, 'executor, E: BlockExecutor, Ba> From<ImportAction<'a, 'executor, E, Ba>> for
	Operation<E::Block, Ba::State, Ba::Auxiliary> where
	Ba: Backend<Block=E::Block> + ?Sized,
	Ba::Auxiliary: Auxiliary<E::Block>,
{
	fn from(
		action: ImportAction<'a, 'executor, E, Ba>
	) -> Operation<E::Block, Ba::State, Ba::Auxiliary> {
		action.pending
	}
}

impl<'a, 'executor, E: BlockExecutor, Ba> ImportAction<'a, 'executor, E, Ba> where
	Ba: Backend<Block=E::Block> + ?Sized,
	Ba::Auxiliary: Auxiliary<E::Block>
{
	/// Swap the backend.
	pub fn swap<Ba2>(self, backend: &'a Ba2) -> ImportAction<'a, 'executor, E, Ba2> where
		Ba2: Backend<Block=E::Block, State=Ba::State, Auxiliary=Ba::Auxiliary> + ?Sized,
		Ba2::Auxiliary: Auxiliary<E::Block>
	{
		ImportAction {
			executor: self.executor,
			backend,
			pending: self.pending,
			_guard: self._guard,
		}
	}
}

impl<'a, 'executor, E: BlockExecutor, Ba> ImportAction<'a, 'executor, E, Ba> where
	Ba: SharedCommittable + Backend<Block=E::Block> + ChainQuery + ?Sized,
	Ba::Auxiliary: Auxiliary<E::Block>,
	Ba::State: AsExternalities<E::Externalities>,
	Error: From<E::Error> + From<Ba::Error>,
{
	/// Create a new import action.
	pub fn new(executor: &'executor E, backend: &'a Ba, import_guard: MutexGuard<'a, ()>) -> Self {
		Self {
			executor, backend,
			pending: Default::default(),
			_guard: import_guard
		}
	}

	/// Get the associated backend of the importer.
	pub fn backend(&self) -> &'a Ba {
		self.backend
	}

	/// Execute a new block.
	pub fn execute_block(
		&self, block: Ba::Block
	) -> Result<ImportOperation<E::Block, Ba::State>, Error> {
		let mut state = self.backend().state_at(
			&block.parent_id().ok_or(Error::IsGenesis)?
		)?;
		self.executor.execute_block(&block, state.as_externalities())?;

		Ok(ImportOperation { block, state })
	}

	/// Import a new block.
	pub fn import_block(&mut self, block: E::Block) -> Result<(), Error> {
		let operation = self.execute_block(block)?;
		self.import_raw(operation);

		Ok(())
	}

	/// Import a raw block.
	pub fn import_raw(&mut self, operation: ImportOperation<E::Block, Ba::State>) {
		self.pending.import_block.push(operation);
	}

	/// Set head to given hash.
	pub fn set_head(&mut self, head: <E::Block as Block>::Identifier) {
		self.pending.set_head = Some(head);
	}

	/// Insert auxiliary value.
	pub fn insert_auxiliary(&mut self, aux: Ba::Auxiliary) {
		self.pending.insert_auxiliaries.push(aux);
	}

	/// Remove auxiliary value.
	pub fn remove_auxiliary(&mut self, aux_key: <Ba::Auxiliary as Auxiliary<E::Block>>::Key) {
		self.pending.remove_auxiliaries.push(aux_key);
	}

	/// Commit operation and drop import lock.
	pub fn commit(self) -> Result<(), Error> {
		Ok(self.backend.commit_action(self)?)
	}
}
