use std::sync::{Arc, RwLock, Mutex, MutexGuard};
use std::marker::PhantomData;
use super::Error;
use crate::backend::{Operation, ImportOperation};
use crate::traits::{IdentifierOf, BlockOf, Block, BlockExecutor, Backend, AsExternalities, BlockContext, AuxiliaryOf, AuxiliaryKeyOf, BlockExecutorOf};

/// A shared backend that also allows atomic import operation.
pub struct SharedBackend<C: BlockContext, B: Backend<C>> {
	backend: Arc<RwLock<B>>,
	import_lock: Arc<Mutex<()>>,
	_marker: PhantomData<C>,
}

impl<C: BlockContext, B> SharedBackend<C, B> where
	B: Backend<C, Operation=Operation<C, B>>
{
	/// Create a new shared backend.
	pub fn new(backend: B) -> Self {
		Self {
			backend: Arc::new(RwLock::new(backend)),
			import_lock: Arc::new(Mutex::new(())),
			_marker: PhantomData,
		}
	}

	/// Get the genesis hash of the chain.
	pub fn genesis(&self) -> IdentifierOf<C> {
		self.backend.read().expect("backend lock is poisoned")
			.genesis()
	}

	/// Get the head of the chain.
	pub fn head(&self) -> IdentifierOf<C> {
		self.backend.read().expect("backend lock is poisoned")
			.head()
	}

	/// Check whether a hash is contained in the chain.
	pub fn contains(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<bool, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.contains(hash)
	}

	/// Check whether a block is canonical.
	pub fn is_canon(
		&self,
		hash: &IdentifierOf<C>
	) -> Result<bool, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.is_canon(hash)
	}

	/// Look up a canonical block via its depth.
	pub fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<IdentifierOf<C>>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.lookup_canon_depth(depth)
	}

	/// Get the auxiliary value by key.
	pub fn auxiliary(
		&self,
		key: &AuxiliaryKeyOf<C>
	) -> Result<Option<AuxiliaryOf<C>>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.auxiliary(key)
	}

	/// Get the depth of a block.
	pub fn depth_at(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<usize, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.depth_at(hash)
	}

	/// Get children of a block.
	pub fn children_at(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<Vec<IdentifierOf<C>>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.children_at(hash)
	}

	/// Get the state object of a block.
	pub fn state_at(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<B::State, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.state_at(hash)
	}

	/// Get the object of a block.
	pub fn block_at(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<BlockOf<C>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.block_at(hash)
	}

	/// Begin an import operation, returns an importer.
	pub fn begin_import<'a, 'executor>(
		&'a self,
		executor: &'executor BlockExecutorOf<C>
	) -> Importer<'a, 'executor, C, B> {
		Importer {
			executor,
			backend: self,
			pending: Default::default(),
			_guard: self.import_lock.lock().expect("Import mutex is poisoned"),
		}
	}
}

impl<C: BlockContext, B: Backend<C>> Clone for SharedBackend<C, B> {
	fn clone(&self) -> Self {
		SharedBackend {
			backend: self.backend.clone(),
			import_lock: self.import_lock.clone(),
			_marker: PhantomData,
		}
	}
}

/// Block importer.
pub struct Importer<'a, 'executor, C: BlockContext, B: Backend<C>> {
	executor: &'executor BlockExecutorOf<C>,
	backend: &'a SharedBackend<C, B>,
	pending: Operation<C, B>,
	_guard: MutexGuard<'a, ()>,
}

impl<'a, 'executor, C: BlockContext, B> Importer<'a, 'executor, C, B> where
	B: Backend<C, Operation=Operation<C, B>>,
{
	/// Get the associated backend of the importer.
	pub fn backend(&self) -> &'a SharedBackend<C, B> {
		self.backend
	}

	/// Import a new block.
	pub fn import_block(&mut self, block: BlockOf<C>) -> Result<(), Error> {
		let mut state = self.backend
			.state_at(&block.parent_id().ok_or(Error::IsGenesis)?)
			.map_err(|e| Error::Backend(Box::new(e)))?;
		self.executor.execute_block(&block, state.as_externalities())
			.map_err(|e| Error::Executor(Box::new(e)))?;

		let operation = ImportOperation { block, state };
		self.import_raw(operation);

		Ok(())
	}

	/// Import a raw block.
	pub fn import_raw(&mut self, operation: ImportOperation<C, B>) {
		self.pending.import_block.push(operation);
	}

	/// Set head to given hash.
	pub fn set_head(&mut self, head: IdentifierOf<C>) {
		self.pending.set_head = Some(head);
	}

	/// Insert auxiliary value.
	pub fn insert_auxiliary(&mut self, aux: AuxiliaryOf<C>) {
		self.pending.insert_auxiliaries.push(aux);
	}

	/// Remove auxiliary value.
	pub fn remove_auxiliary(&mut self, aux_key: AuxiliaryKeyOf<C>) {
		self.pending.remove_auxiliaries.push(aux_key);
	}

	/// Commit operation and drop import lock.
	pub fn commit(self) -> Result<(), B::Error> {
		self.backend.backend.write().expect("backend lock is poisoned")
			.commit(self.pending)
	}
}
