use std::sync::{Arc, RwLock, Mutex, MutexGuard};
use std::marker::PhantomData;
use super::{Error, Operation, ImportOperation};
use crate::traits::{HashOf, BlockOf, Block, BlockExecutor, Backend, AsExternalities, AuxiliaryContext, AuxiliaryOf, AuxiliaryKeyOf, TagOf};

pub struct SharedBackend<C: AuxiliaryContext, B: Backend<C>> {
	backend: Arc<RwLock<B>>,
	import_lock: Arc<Mutex<()>>,
	_marker: PhantomData<C>,
}

impl<C: AuxiliaryContext, B> SharedBackend<C, B> where
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

	pub fn is_canon(
		&self,
		hash: &HashOf<C>
	) -> Result<bool, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.is_canon(hash)
	}

	pub fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<HashOf<C>>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.lookup_canon_depth(depth)
	}

	pub fn lookup_canon_tag(
		&self,
		tag: &TagOf<C>,
	) -> Result<Option<HashOf<C>>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.lookup_canon_tag(tag)
	}

	pub fn auxiliary(
		&self,
		key: &AuxiliaryKeyOf<C>
	) -> Result<Option<AuxiliaryOf<C>>, B::Error> {
		self.backend.read().expect("backend lock is poisoned")
			.auxiliary(key)
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

impl<C: AuxiliaryContext, B: Backend<C>> Clone for SharedBackend<C, B> {
	fn clone(&self) -> Self {
		SharedBackend {
			backend: self.backend.clone(),
			import_lock: self.import_lock.clone(),
			_marker: PhantomData,
		}
	}
}

pub struct Importer<'a, 'executor, C: AuxiliaryContext, B: Backend<C>, E> {
	executor: &'executor E,
	backend: &'a SharedBackend<C, B>,
	pending: Operation<C, B>,
	_guard: MutexGuard<'a, ()>,
}

impl<'a, 'executor, C: AuxiliaryContext, B, E> Importer<'a, 'executor, C, B, E> where
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
		self.import_raw(operation);

		Ok(())
	}

	pub fn import_raw(&mut self, operation: ImportOperation<C, B>) {
		self.pending.import_block.push(operation);
	}

	pub fn set_head(&mut self, head: HashOf<C>) {
		self.pending.set_head = Some(head);
	}

	pub fn insert_auxiliary(&mut self, aux: AuxiliaryOf<C>) {
		self.pending.insert_auxiliaries.push(aux);
	}

	pub fn remove_auxiliary(&mut self, aux_key: AuxiliaryKeyOf<C>) {
		self.pending.remove_auxiliaries.push(aux_key);
	}

	pub fn commit(self) -> Result<(), B::Error> {
		self.backend.backend.write().expect("backend lock is poisoned")
			.commit(self.pending)
	}
}
