use std::sync::{Arc, RwLock, Mutex, MutexGuard};
use std::marker::PhantomData;
use super::{Error, Operation, ImportOperation};
use crate::traits::{HashOf, BlockOf, Block, BlockExecutor, Backend, BaseContext, AsExternalities};

pub struct SharedBackend<C: BaseContext, B: Backend<C>> {
	backend: Arc<RwLock<B>>,
	import_lock: Arc<Mutex<()>>,
	_marker: PhantomData<C>,
}

impl<C: BaseContext, B> SharedBackend<C, B> where
	B: Backend<C, Operation=Operation<C, B>>
{
	pub fn new(backend: B) -> Self {
		Self {
			backend: Arc::new(RwLock::new(backend)),
			import_lock: Arc::new(Mutex::new(())),
			_marker: PhantomData,
		}
	}

	pub fn genesis(&self) -> HashOf<C> {
		self.backend.read().expect("backend lock is poisoned")
			.genesis()
	}

	pub fn head(&self) -> HashOf<C> {
		self.backend.read().expect("backend lock is poisoned")
			.head()
	}

	pub fn contains(
		&self,
		hash: &HashOf<C>,
	) -> Result<bool, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.contains(hash)
	}

	pub fn depth_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<usize, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.depth_at(hash)
	}

	pub fn children_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Vec<HashOf<C>>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.children_at(hash)
	}

	pub fn state_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<B::State, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.state_at(hash)
	}

	pub fn block_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<BlockOf<C>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.block_at(hash)
	}

	pub fn begin_import<'a, 'executor, E: BlockExecutor<C>>(
		&'a self,
		executor: &'executor E
	) -> Importer<'a, 'executor, C, B, E> {
		Importer {
			executor,
			backend: self,
			pending: Default::default(),
			_guard: self.import_lock.lock().expect("Import mutex is poisoned"),
		}
	}
}

impl<C: BaseContext, B: Backend<C>> Clone for SharedBackend<C, B> {
	fn clone(&self) -> Self {
		SharedBackend {
			backend: self.backend.clone(),
			import_lock: self.import_lock.clone(),
			_marker: PhantomData,
		}
	}
}

pub struct Importer<'a, 'executor, C: BaseContext, B: Backend<C>, E> {
	executor: &'executor E,
	backend: &'a SharedBackend<C, B>,
	pending: Operation<C, B>,
	_guard: MutexGuard<'a, ()>,
}

impl<'a, 'executor, C: BaseContext, B, E> Importer<'a, 'executor, C, B, E> where
	B: Backend<C, Operation=Operation<C, B>>,
	E: BlockExecutor<C>,
{
	pub fn backend(&self) -> &'a SharedBackend<C, B> {
		self.backend
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

	pub fn commit(self) -> Result<(), B::Error> {
		self.backend.backend.write().expect("backend lock is poisoned")
			.commit(self.pending)
	}
}
