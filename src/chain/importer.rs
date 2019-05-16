use std::sync::{Arc, RwLock, Mutex, MutexGuard};
use std::ops::Deref;
use super::Error;
use crate::traits::{Operation, ImportOperation, Block, BlockExecutor, Backend, AsExternalities, Auxiliary, ChainQuery};

/// A shared backend that also allows atomic import operation.
pub struct SharedBackend<Ba: Backend> {
	backend: Arc<RwLock<Ba>>,
	import_lock: Arc<Mutex<()>>,
}

impl<Ba: Backend> SharedBackend<Ba> {
	/// Create a new shared backend.
	pub fn new(backend: Ba) -> Self {
		Self {
			backend: Arc::new(RwLock::new(backend)),
			import_lock: Arc::new(Mutex::new(())),
		}
	}

	/// Return a read handle of the backend.
	pub fn read<'a>(&'a self) -> impl Deref<Target=Ba> + 'a {
		self.backend.read().expect("backend lock is poisoned")
	}

	/// Begin an import operation, returns an importer.
	pub fn begin_import<'a, 'executor, E: BlockExecutor<Block=Ba::Block>>(
		&'a self,
		executor: &'executor E
	) -> Importer<'a, 'executor, E, Ba> where
		Ba::State: AsExternalities<E::Externalities>
	{
		Importer {
			executor,
			backend: self,
			pending: Default::default(),
			_guard: self.import_lock.lock().expect("Import mutex is poisoned"),
		}
	}
}

impl<Ba: Backend> Clone for SharedBackend<Ba> {
	fn clone(&self) -> Self {
		SharedBackend {
			backend: self.backend.clone(),
			import_lock: self.import_lock.clone(),
		}
	}
}

/// Block importer.
pub struct Importer<'a, 'executor, E: BlockExecutor, Ba: Backend<Block=E::Block>> where
	Ba::Auxiliary: Auxiliary<E::Block>
{
	executor: &'executor E,
	backend: &'a SharedBackend<Ba>,
	pending: Operation<E::Block, Ba::State, Ba::Auxiliary>,
	_guard: MutexGuard<'a, ()>,
}

impl<'a, 'executor, E: BlockExecutor, Ba> Importer<'a, 'executor, E, Ba> where
	Ba: Backend<Block=E::Block> + ChainQuery,
	Ba::Auxiliary: Auxiliary<E::Block>,
	Ba::State: AsExternalities<E::Externalities>,
	Error: From<E::Error> + From<Ba::Error>,
{
	/// Get the associated backend of the importer.
	pub fn backend(&self) -> &'a SharedBackend<Ba> {
		self.backend
	}

	/// Execute a new block.
	pub fn execute_block(&self, block: E::Block) -> Result<ImportOperation<E::Block, Ba::State>, Error> {
		let mut state = self.backend
			.read()
			.state_at(&block.parent_id().ok_or(Error::IsGenesis)?)?;
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
		Ok(self.backend.backend.write().expect("backend lock is poisoned")
		   .commit(self.pending)?)
	}
}
