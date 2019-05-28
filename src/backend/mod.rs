//! Basic backend definitions and memory backend.

mod memory;
mod route;

pub use self::memory::{KeyValueMemoryState, MemoryBackend, MemoryLikeBackend, Error as MemoryError};
pub use self::route::{tree_route, TreeRoute};
// pub use crate::import::SharedBackend;

use std::sync::{Arc, RwLock, Mutex};
use crate::import::ImportAction;
use crate::traits::{Backend, BlockExecutor, AsExternalities, ChainQuery, Operation, Block, Auxiliary};

/// Committable backend.
pub trait Committable: Backend {
	/// Commit operation.
	fn commit(
		&mut self,
		operation: Operation<Self::Block, Self::State, Self::Auxiliary>,
	) -> Result<(), Self::Error>;
}

/// Actionable backend.
pub trait Actionable: Backend {
	/// Begin an import operation, returns an importer.
	fn begin_action<'a, 'executor, E: BlockExecutor<Block=Self::Block>>(
		&'a self,
		executor: &'executor E
	) -> ImportAction<'a, 'executor, E, Self> where
		crate::import::Error: From<E::Error> + From<Self::Error>,
		Self::State: AsExternalities<E::Externalities>;

	/// Commit an action into the backend.
	fn commit_action<'a, 'executor, E: BlockExecutor<Block=Self::Block>>(
		&'a self,
		action: ImportAction<'a, 'executor, E, Self>
	) -> Result<(), Self::Error> where
		Self::State: AsExternalities<E::Externalities>;
}

/// A shared backend based on RwLock.
pub struct RwLockBackend<Ba: Backend> {
	backend: Arc<RwLock<Ba>>,
	import_lock: Arc<Mutex<()>>,
}

impl<Ba: Backend> RwLockBackend<Ba> {
	/// Create a new shared RwLock-based backend.
	pub fn new(backend: Ba) -> Self {
		Self {
			backend: Arc::new(RwLock::new(backend)),
			import_lock: Arc::new(Mutex::new(())),
		}
	}
}

impl<Ba: Backend> Backend for RwLockBackend<Ba> {
	type Block = Ba::Block;
	type State = Ba::State;
	type Auxiliary = Ba::Auxiliary;
	type Error = Ba::Error;
}

impl<Ba: ChainQuery> ChainQuery for RwLockBackend<Ba> {
	fn genesis(&self) -> <Self::Block as Block>::Identifier {
		self.backend.read().expect("Lock is poisoned").genesis()
	}
	fn head(&self) -> <Self::Block as Block>::Identifier {
		self.backend.read().expect("Lock is poisoned").head()
	}
	fn contains(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		self.backend.read().expect("Lock is poisoned").contains(hash)
	}
	fn is_canon(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		self.backend.read().expect("Lock is poisoned").is_canon(hash)
	}
	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<<Self::Block as Block>::Identifier>, Self::Error> {
		self.backend.read().expect("Lock is poisoned").lookup_canon_depth(depth)
	}
	fn auxiliary(
		&self,
		key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
	) -> Result<Option<Self::Auxiliary>, Self::Error> {
		self.backend.read().expect("Lock is poisoned").auxiliary(key)
	}
	fn depth_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<usize, Self::Error> {
		self.backend.read().expect("Lock is poisoned").depth_at(hash)
	}
	fn children_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Vec<<Self::Block as Block>::Identifier>, Self::Error> {
		self.backend.read().expect("Lock is poisoned").children_at(hash)
	}
	fn state_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::State, Self::Error> {
		self.backend.read().expect("Lock is poisoned").state_at(hash)
	}
	fn block_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::Block, Self::Error> {
		self.backend.read().expect("Lock is poisoned").block_at(hash)
	}
}

impl<Ba: Committable + ChainQuery> Actionable for RwLockBackend<Ba> {
	fn begin_action<'a, 'executor, E: BlockExecutor<Block=Self::Block>>(
		&'a self,
		executor: &'executor E
	) -> ImportAction<'a, 'executor, E, Self> where
		crate::import::Error: From<E::Error> + From<Self::Error>,
		Self::State: AsExternalities<E::Externalities>
	{
		ImportAction::new(executor, &self, self.import_lock.lock().expect("Lock is poisoned"))
	}

	fn commit_action<'a, 'executor, E: BlockExecutor<Block=Self::Block>>(
		&'a self,
		action: ImportAction<'a, 'executor, E, Self>
	) -> Result<(), Self::Error> where
		Self::State: AsExternalities<E::Externalities>
	{
		self.backend.write().expect("Lock is poisoned")
			.commit(action.into())
	}
}
