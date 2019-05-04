use std::sync::{Arc, RwLock, Mutex, MutexGuard};
use std::ops::Deref;
use std::marker::PhantomData;
use super::Error;
use crate::traits::{Operation, ImportOperation, Block, BlockExecutor, Backend, AsExternalities, Auxiliary, ChainQuery};

/// A shared backend that also allows atomic import operation.
pub struct SharedBackend<B: Block, A: Auxiliary<B>, Ba: Backend<B, A>> {
	backend: Arc<RwLock<Ba>>,
	import_lock: Arc<Mutex<()>>,
	_marker: PhantomData<(B, A)>,
}

impl<B: Block, A: Auxiliary<B>, Ba> SharedBackend<B, A, Ba> where
	Ba: Backend<B, A>
{
	/// Create a new shared backend.
	pub fn new(backend: Ba) -> Self {
		Self {
			backend: Arc::new(RwLock::new(backend)),
			import_lock: Arc::new(Mutex::new(())),
			_marker: PhantomData,
		}
	}

	/// Return a read handle of the backend.
	pub fn read<'a>(&'a self) -> impl Deref<Target=Ba> + 'a {
		self.backend.read().expect("backend lock is poisoned")
	}

	/// Begin an import operation, returns an importer.
	pub fn begin_import<'a, 'executor, E: BlockExecutor<Block=B>>(
		&'a self,
		executor: &'executor E
	) -> Importer<'a, 'executor, E, A, Ba> where
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

impl<B: Block, A: Auxiliary<B>, Ba: Backend<B, A>> Clone for SharedBackend<B, A, Ba> {
	fn clone(&self) -> Self {
		SharedBackend {
			backend: self.backend.clone(),
			import_lock: self.import_lock.clone(),
			_marker: PhantomData,
		}
	}
}

/// Block importer.
pub struct Importer<'a, 'executor, E: BlockExecutor, A: Auxiliary<E::Block>, Ba: Backend<E::Block, A>> {
	executor: &'executor E,
	backend: &'a SharedBackend<E::Block, A, Ba>,
	pending: Operation<E::Block, Ba::State, A>,
	_guard: MutexGuard<'a, ()>,
}

impl<'a, 'executor, E: BlockExecutor, A: Auxiliary<E::Block>, Ba: ChainQuery<E::Block, A>> Importer<'a, 'executor, E, A, Ba> where
	<Ba as Backend<E::Block, A>>::State: AsExternalities<E::Externalities>,
{
	/// Get the associated backend of the importer.
	pub fn backend(&self) -> &'a SharedBackend<E::Block, A, Ba> {
		self.backend
	}

	/// Import a new block.
	pub fn import_block(&mut self, block: E::Block) -> Result<(), Error> {
		let mut state = self.backend
			.read()
			.state_at(&block.parent_id().ok_or(Error::IsGenesis)?)
			.map_err(|e| Error::Backend(Box::new(e)))?;
		self.executor.execute_block(&block, state.as_externalities())
			.map_err(|e| Error::Executor(Box::new(e)))?;

		let operation = ImportOperation { block, state };
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
	pub fn insert_auxiliary(&mut self, aux: A) {
		self.pending.insert_auxiliaries.push(aux);
	}

	/// Remove auxiliary value.
	pub fn remove_auxiliary(&mut self, aux_key: A::Key) {
		self.pending.remove_auxiliaries.push(aux_key);
	}

	/// Commit operation and drop import lock.
	pub fn commit(self) -> Result<(), Ba::Error> {
		self.backend.backend.write().expect("backend lock is poisoned")
			.commit(self.pending)
	}
}
