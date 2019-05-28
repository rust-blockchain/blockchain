use std::{fmt, error as stderror};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use crate::traits::{Backend, Operation, AsExternalities, BlockExecutor};
use crate::import::ImportAction;
use crate::backend::{tree_route, Committable, SharedCommittable, Database, SharedDatabase, ChainQuery, Block, Auxiliary};

#[derive(Debug)]
/// Memory errors
pub enum Error {
	/// Invalid operation.
	InvalidOperation,
	/// Trying to import a block that is genesis.
	IsGenesis,
	/// Block trying to query does not exist in the backend.
	NotExist,
	/// Underlying database error.
	Database(Box<stderror::Error>),
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl stderror::Error for Error { }

impl From<Error> for crate::import::Error {
	fn from(error: Error) -> Self {
		crate::import::Error::Backend(Box::new(error))
	}
}

/// Representing raw block data.
pub struct BlockData<B: Block, S> {
	/// Block of the data.
	pub block: B,
	/// Block state.
	pub state: S,
	/// Depth.
	pub depth: usize,
	/// Children of the current block.
	pub children: Vec<B::Identifier>,
	/// Whether the block is on the canonical chain.
	pub is_canon: bool,
}

/// Direct backend built on top of a database.
pub struct DirectBackend<DB: Database> {
	db: DB
}

/// Shared direct backend built on top of a shared database.
pub struct SharedDirectBackend<DB: SharedDatabase> {
	db: DB,
	import_lock: Arc<Mutex<()>>,
}

impl<DB: SharedDatabase> Clone for SharedDirectBackend<DB> {
	fn clone(&self) -> Self {
		Self {
			db: self.db.clone(),
			import_lock: self.import_lock.clone(),
		}
	}
}

impl<DB: Database> Backend for DirectBackend<DB> {
	type Block = DB::Block;
	type State = DB::State;
	type Auxiliary = DB::Auxiliary;
	type Error = Error;
}

impl<DB: SharedDatabase> Backend for SharedDirectBackend<DB> {
	type Block = DB::Block;
	type State = DB::State;
	type Auxiliary = DB::Auxiliary;
	type Error = Error;
}

macro_rules! commit_operation {
	( $self:ident, $operation:ident ) => {
		{
			let mut parent_ides = HashMap::new();
			let mut importing: HashMap<<Self::Block as Block>::Identifier, BlockData<Self::Block, Self::State>> = HashMap::new();
			let mut verifying = $operation.import_block;

			// Do precheck to make sure the import $operation is valid.
			loop {
				let mut progress = false;
				let mut next_verifying = Vec::new();

				for op in verifying {
					let parent_depth = match op.block.parent_id() {
						Some(parent_id) => {
							if $self.contains(&parent_id)? {
								Some($self.depth_at(&parent_id)?)
							} else if importing.contains_key(&parent_id) {
								importing.get(&parent_id)
									.map(|data| data.depth)
							} else {
								None
							}
						},
						None => return Err(Error::IsGenesis),
					};
					let depth = parent_depth.map(|d| d + 1);

					if let Some(depth) = depth {
						progress = true;
						if let Some(parent_id) = op.block.parent_id() {
							parent_ides.insert(op.block.id(), parent_id);
						}
						importing.insert(op.block.id(), BlockData {
							block: op.block,
							state: op.state,
							depth,
							children: Vec::new(),
							is_canon: false,
						});
					} else {
						next_verifying.push(op)
					}
				}

				if next_verifying.len() == 0 {
					break;
				}

				if !progress {
					return Err(Error::InvalidOperation);
				}

				verifying = next_verifying;
			}

			// Do precheck to make sure the head going to set exists.
			if let Some(new_head) = &$operation.set_head {
				let head_exists = $self.contains(new_head)? ||
					importing.contains_key(new_head);

				if !head_exists {
					return Err(Error::InvalidOperation);
				}
			}

			// Do precheck to make sure auxiliary is valid.
			for aux in &$operation.insert_auxiliaries {
				for id in aux.associated() {
					if !($self.contains(&id)? || importing.contains_key(&id)) {
						return Err(Error::InvalidOperation);
					}
				}
			}

			for (id, data) in importing {
				$self.db.insert_block(
					id, data.block, data.state, data.depth, data.children, data.is_canon
				);
			}

			// Fix children at ides.
			for (id, parent_id) in parent_ides {
				$self.db.push_child(parent_id, id);
			}

			if let Some(new_head) = $operation.set_head {
				let route = tree_route($self, &$self.head(), &new_head)
					.expect("Blocks are checked to exist or importing; qed");

				for id in route.retracted() {
					$self.db.set_canon(*id, false);
					let depth = $self.depth_at(id)
						.expect("Block is fetched from tree_route; it must exist; qed");
					$self.db.remove_canon_depth_mapping(&depth);
				}

				for id in route.enacted() {
					$self.db.set_canon(*id, true);
					let depth = $self.depth_at(id)
						.expect("Block is fetched from tree_route; it must exist; qed");
					$self.db.insert_canon_depth_mapping(depth, *id);
				}

				$self.db.set_head(new_head);
			}

			for aux_key in $operation.remove_auxiliaries {
				$self.db.remove_auxiliary(&aux_key);
			}

			for aux in $operation.insert_auxiliaries {
				$self.db.insert_auxiliary(aux.key(), aux);
			}

			Ok(())
		}
	}
}

impl<DB: Database + ChainQuery> Committable for DirectBackend<DB> where
	Error: From<DB::Error>
{
	fn commit(
		&mut self,
		operation: Operation<DB::Block, DB::State, DB::Auxiliary>
	) -> Result<(), Error> {
		commit_operation!(self, operation)
	}
}

impl<DB: SharedDatabase + ChainQuery> SharedCommittable for SharedDirectBackend<DB> where
	Error: From<DB::Error>,
	crate::import::Error: From<DB::Error>,
{
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
		let operation = Operation::from(action);
		commit_operation!(self, operation)
	}

	fn lock_import<'a>(&'a self) -> MutexGuard<'a, ()> {
		self.import_lock.lock().expect("Lock is poisoned")
	}
}

impl<DB: Database + ChainQuery> ChainQuery for DirectBackend<DB> where
	Error: From<DB::Error>
{
	fn genesis(&self) -> <Self::Block as Block>::Identifier {
		self.db.genesis()
	}
	fn head(&self) -> <Self::Block as Block>::Identifier {
		self.db.head()
	}
	fn contains(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		Ok(self.db.contains(hash)?)
	}
	fn is_canon(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		Ok(self.db.is_canon(hash)?)
	}
	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<<Self::Block as Block>::Identifier>, Self::Error> {
		Ok(self.db.lookup_canon_depth(depth)?)
	}
	fn auxiliary(
		&self,
		key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
	) -> Result<Option<Self::Auxiliary>, Self::Error> {
		Ok(self.db.auxiliary(key)?)
	}
	fn depth_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<usize, Self::Error> {
		Ok(self.db.depth_at(hash)?)
	}
	fn children_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Vec<<Self::Block as Block>::Identifier>, Self::Error> {
		Ok(self.db.children_at(hash)?)
	}
	fn state_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::State, Self::Error> {
		Ok(self.db.state_at(hash)?)
	}
	fn block_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::Block, Self::Error> {
		Ok(self.db.block_at(hash)?)
	}
}

impl<DB: SharedDatabase + ChainQuery> ChainQuery for SharedDirectBackend<DB> where
	Error: From<DB::Error>
{
	fn genesis(&self) -> <Self::Block as Block>::Identifier {
		self.db.genesis()
	}
	fn head(&self) -> <Self::Block as Block>::Identifier {
		self.db.head()
	}
	fn contains(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		Ok(self.db.contains(hash)?)
	}
	fn is_canon(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		Ok(self.db.is_canon(hash)?)
	}
	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<<Self::Block as Block>::Identifier>, Self::Error> {
		Ok(self.db.lookup_canon_depth(depth)?)
	}
	fn auxiliary(
		&self,
		key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
	) -> Result<Option<Self::Auxiliary>, Self::Error> {
		Ok(self.db.auxiliary(key)?)
	}
	fn depth_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<usize, Self::Error> {
		Ok(self.db.depth_at(hash)?)
	}
	fn children_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Vec<<Self::Block as Block>::Identifier>, Self::Error> {
		Ok(self.db.children_at(hash)?)
	}
	fn state_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::State, Self::Error> {
		Ok(self.db.state_at(hash)?)
	}
	fn block_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::Block, Self::Error> {
		Ok(self.db.block_at(hash)?)
	}
}

impl<DB: Database> DirectBackend<DB> {
	/// Create a new direct backend from an existing database.
	pub fn new(existing: DB) -> Self {
		Self {
			db: existing
		}
	}
}

impl<DB: SharedDatabase> SharedDirectBackend<DB> {
	/// Create a new shared direct backend from an existing database.
	pub fn new(existing: DB) -> Self {
		Self {
			db: existing,
			import_lock: Arc::new(Mutex::new(())),
		}
	}
}
