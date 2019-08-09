use std::{fmt, error as stderror};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::{Block, Auxiliary};
use crate::backend::{Store, BlockData, ChainQuery, ChainSettlement, Operation, Committable, SharedCommittable, OperationError};

#[derive(Debug)]
/// Memory errors
pub enum Error {
	/// Invalid Operation
	InvalidOperation,
	/// Trying to import a block that is genesis
	IsGenesis,
	/// Query does not exist
	NotExist,
}

impl OperationError for Error {
	fn invalid_operation() -> Self {
		Error::InvalidOperation
	}

	fn block_is_genesis() -> Self {
		Error::IsGenesis
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl stderror::Error for Error { }

/// Database backed by memory.
pub struct MemoryDatabase<B: Block, A: Auxiliary<B>, S> {
	blocks_and_states: HashMap<B::Identifier, BlockData<B, S>>,
	head: B::Identifier,
	genesis: B::Identifier,
	canon_depth_mappings: HashMap<usize, B::Identifier>,
	auxiliaries: HashMap<A::Key, A>,
}

impl<B: Block, A: Auxiliary<B>, S: Clone> Store for MemoryDatabase<B, A, S> {
	type Block = B;
	type State = S;
	type Auxiliary = A;
	type Error = Error;
}

impl<B: Block, A: Auxiliary<B>, S: Clone> ChainQuery for MemoryDatabase<B, A, S> {
	fn head(&self) -> B::Identifier { self.head.clone() }
	fn genesis(&self) -> B::Identifier { self.genesis.clone() }

	fn contains(
		&self,
		id: &B::Identifier
	) -> Result<bool, Error> {
		Ok(self.blocks_and_states.contains_key(id))
	}

	fn is_canon(
		&self,
		id: &B::Identifier
	) -> Result<bool, Error> {
		self.blocks_and_states.get(id)
			.map(|data| data.is_canon)
			.ok_or(Error::NotExist)
	}

	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<B::Identifier>, Error> {
		Ok(self.canon_depth_mappings.get(&depth)
		   .map(|h| h.clone()))
	}

	fn auxiliary(
		&self,
		key: &A::Key
	) -> Result<Option<A>, Error> {
		Ok(self.auxiliaries.get(key).map(|v| v.clone()))
	}

	fn children_at(
		&self,
		id: &B::Identifier,
	) -> Result<Vec<B::Identifier>, Error> {
		self.blocks_and_states.get(id)
			.map(|data| data.children.clone())
			.ok_or(Error::NotExist)
	}

	fn depth_at(
		&self,
		id: &B::Identifier
	) -> Result<usize, Error> {
		self.blocks_and_states.get(id)
		   .map(|data| data.depth)
		   .ok_or(Error::NotExist)
	}

	fn block_at(
		&self,
		id: &B::Identifier,
	) -> Result<B, Error> {
		self.blocks_and_states.get(id)
			.map(|data| data.block.clone())
			.ok_or(Error::NotExist)
	}

	fn state_at(
		&self,
		id: &B::Identifier,
	) -> Result<Self::State, Error> {
		self.blocks_and_states.get(id)
			.map(|data| data.state.clone())
			.ok_or(Error::NotExist)
	}
}

impl<B: Block, A: Auxiliary<B>, S: Clone> ChainSettlement for MemoryDatabase<B, A, S> {
	fn insert_block(
		&mut self,
		id: <Self::Block as Block>::Identifier,
		block: Self::Block,
		state: Self::State,
		depth: usize,
		children: Vec<<Self::Block as Block>::Identifier>,
		is_canon: bool
	) {
		self.blocks_and_states.insert(id, BlockData {
			block, state, depth, children, is_canon
		});
	}
	fn push_child(
		&mut self,
		id: <Self::Block as Block>::Identifier,
		child: <Self::Block as Block>::Identifier,
	) {
		self.blocks_and_states.get_mut(&id)
			.expect("Internal database error")
			.children.push(child);
	}
	fn set_canon(
		&mut self,
		id: <Self::Block as Block>::Identifier,
		is_canon: bool
	) {
		self.blocks_and_states.get_mut(&id)
			.expect("Internal database error")
			.is_canon = is_canon;
	}
	fn insert_canon_depth_mapping(
		&mut self,
		depth: usize,
		id: <Self::Block as Block>::Identifier,
	) {
		self.canon_depth_mappings.insert(depth, id);
	}
	fn remove_canon_depth_mapping(
		&mut self,
		depth: &usize
	) {
		self.canon_depth_mappings.remove(depth);
	}
	fn insert_auxiliary(
		&mut self,
		key: <Self::Auxiliary as Auxiliary<Self::Block>>::Key,
		value: Self::Auxiliary
	) {
		self.auxiliaries.insert(key, value);
	}
	fn remove_auxiliary(
		&mut self,
		key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
	) {
		self.auxiliaries.remove(key);
	}
	fn set_head(
		&mut self,
		head: <Self::Block as Block>::Identifier
	) {
		self.head = head;
	}
}

/// Memory backend
pub struct MemoryBackend<B: Block, A: Auxiliary<B>, S>(MemoryDatabase<B, A, S>);

impl<B: Block, A: Auxiliary<B>, S: Clone> MemoryBackend<B, A, S> {
	/// Create a new memory backend from genesis.
	pub fn new_with_genesis(block: B, genesis_state: S) -> Self {
		assert!(block.parent_id().is_none(), "with_genesis must be provided with a genesis block");

		let genesis_id = block.id();
		let mut blocks_and_states = HashMap::new();
		blocks_and_states.insert(
			block.id(),
			BlockData {
				block,
				state: genesis_state,
				depth: 0,
				children: Vec::new(),
				is_canon: true,
			}
		);
		let mut canon_depth_mappings = HashMap::new();
		canon_depth_mappings.insert(0, genesis_id.clone());

		Self(MemoryDatabase {
			blocks_and_states,
			canon_depth_mappings,
			auxiliaries: Default::default(),
			genesis: genesis_id.clone(),
			head: genesis_id,
		})
	}
}

impl<B: Block, A: Auxiliary<B>, S: Clone> Store for MemoryBackend<B, A, S> {
	type Block = B;
	type State = S;
	type Auxiliary = A;
	type Error = Error;
}

impl<B: Block, A: Auxiliary<B>, S: Clone> ChainQuery for MemoryBackend<B, A, S> {
	fn genesis(&self) -> <Self::Block as Block>::Identifier {
		self.0.genesis()
	}
	fn head(&self) -> <Self::Block as Block>::Identifier {
		self.0.head()
	}
	fn contains(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		Ok(self.0.contains(hash)?)
	}
	fn is_canon(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		Ok(self.0.is_canon(hash)?)
	}
	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<<Self::Block as Block>::Identifier>, Self::Error> {
		Ok(self.0.lookup_canon_depth(depth)?)
	}
	fn auxiliary(
		&self,
		key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
	) -> Result<Option<Self::Auxiliary>, Self::Error> {
		Ok(self.0.auxiliary(key)?)
	}
	fn depth_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<usize, Self::Error> {
		Ok(self.0.depth_at(hash)?)
	}
	fn children_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Vec<<Self::Block as Block>::Identifier>, Self::Error> {
		Ok(self.0.children_at(hash)?)
	}
	fn state_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::State, Self::Error> {
		Ok(self.0.state_at(hash)?)
	}
	fn block_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::Block, Self::Error> {
		Ok(self.0.block_at(hash)?)
	}
}

impl<B: Block, A: Auxiliary<B>, S: Clone> Committable for MemoryBackend<B, A, S> {
	type Operation = Operation<Self::Block, Self::State, Self::Auxiliary>;

	fn commit(
		&mut self,
		operation: Operation<Self::Block, Self::State, Self::Auxiliary>,
	) -> Result<(), Self::Error> {
		operation.settle(&mut self.0)
	}
}

/// Shared memory backend
pub struct SharedMemoryBackend<B: Block, A: Auxiliary<B>, S>(
	Arc<RwLock<MemoryBackend<B, A, S>>>
);

impl<B: Block, A: Auxiliary<B>, S: Clone> SharedMemoryBackend<B, A, S> {
	/// Create a new memory backend from genesis.
	pub fn new_with_genesis(block: B, genesis_state: S) -> Self {
		Self(Arc::new(RwLock::new(MemoryBackend::new_with_genesis(block, genesis_state))))
	}
}

impl<B: Block, A: Auxiliary<B>, S: Clone> Store for SharedMemoryBackend<B, A, S> {
	type Block = B;
	type State = S;
	type Auxiliary = A;
	type Error = Error;
}

impl<B: Block, A: Auxiliary<B>, S: Clone> ChainQuery for SharedMemoryBackend<B, A, S> {
	fn genesis(&self) -> <Self::Block as Block>::Identifier {
		self.0.read().expect("Lock is poisoned").genesis()
	}
	fn head(&self) -> <Self::Block as Block>::Identifier {
		self.0.read().expect("Lock is poisoned").head()
	}
	fn contains(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		Ok(self.0.read().expect("Lock is poisoned").contains(hash)?)
	}
	fn is_canon(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error> {
		Ok(self.0.read().expect("Lock is poisoned").is_canon(hash)?)
	}
	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<<Self::Block as Block>::Identifier>, Self::Error> {
		Ok(self.0.read().expect("Lock is poisoned").lookup_canon_depth(depth)?)
	}
	fn auxiliary(
		&self,
		key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
	) -> Result<Option<Self::Auxiliary>, Self::Error> {
		Ok(self.0.read().expect("Lock is poisoned").auxiliary(key)?)
	}
	fn depth_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<usize, Self::Error> {
		Ok(self.0.read().expect("Lock is poisoned").depth_at(hash)?)
	}
	fn children_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Vec<<Self::Block as Block>::Identifier>, Self::Error> {
		Ok(self.0.read().expect("Lock is poisoned").children_at(hash)?)
	}
	fn state_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::State, Self::Error> {
		Ok(self.0.read().expect("Lock is poisoned").state_at(hash)?)
	}
	fn block_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::Block, Self::Error> {
		Ok(self.0.read().expect("Lock is poisoned").block_at(hash)?)
	}
}

impl<B: Block, A: Auxiliary<B>, S: Clone> Clone for SharedMemoryBackend<B, A, S> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<B: Block, A: Auxiliary<B>, S: Clone> SharedCommittable for SharedMemoryBackend<B, A, S> {
	type Operation = Operation<Self::Block, Self::State, Self::Auxiliary>;

	fn commit(
		&self,
		operation: Operation<Self::Block, Self::State, Self::Auxiliary>,
	) -> Result<(), Self::Error> {
		self.0.write().expect("Lock is poisoned").commit(operation)
	}
}
