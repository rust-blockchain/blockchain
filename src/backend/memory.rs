use std::collections::HashMap;
use std::{fmt, error as stderror};
use std::convert::Infallible;

use crate::traits::{
	AsExternalities, Backend, NullExternalities,
	StorageExternalities, Block, Auxiliary,
	ChainQuery,
};
use super::{BlockData, Database, DirectBackend};

#[derive(Debug)]
/// Memory errors
pub enum Error {
	/// Block trying to query does not exist in the backend.
	NotExist,
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

impl From<Error> for crate::backend::DirectError {
	fn from(error: Error) -> Self {
		match error {
			Error::NotExist => crate::backend::DirectError::NotExist,
		}
	}
}

/// A backend type that stores all information in memory.
pub trait MemoryLikeBackend: Backend {
	/// Create a new memory backend from a genesis block.
	fn new_with_genesis(block: Self::Block, genesis_state: Self::State) -> Self;
}

impl<DB: Database + MemoryLikeBackend> MemoryLikeBackend for DirectBackend<DB> {
	fn new_with_genesis(block: Self::Block, genesis_state: Self::State) -> Self {
		DirectBackend::new(DB::new_with_genesis(block, genesis_state))
	}
}

/// State stored in memory.
#[derive(Clone, Default)]
pub struct KeyValueMemoryState {
	storage: HashMap<Vec<u8>, Vec<u8>>,
}

impl AsRef<HashMap<Vec<u8>, Vec<u8>>> for KeyValueMemoryState {
	fn as_ref(&self) -> &HashMap<Vec<u8>, Vec<u8>> {
		&self.storage
	}
}

impl AsMut<HashMap<Vec<u8>, Vec<u8>>> for KeyValueMemoryState {
	fn as_mut(&mut self) -> &mut HashMap<Vec<u8>, Vec<u8>> {
		&mut self.storage
	}
}

impl NullExternalities for KeyValueMemoryState { }

impl AsExternalities<dyn NullExternalities> for KeyValueMemoryState {
	fn as_externalities(&mut self) -> &mut (dyn NullExternalities + 'static) {
		self
	}
}

impl StorageExternalities<Infallible> for KeyValueMemoryState {
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Infallible> {
		Ok(self.storage.get(key).map(|value| value.to_vec()))
	}

	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>) {
		self.storage.insert(key, value);
	}

	fn remove_storage(&mut self, key: &[u8]) {
		self.storage.remove(key);
	}
}

impl StorageExternalities<Box<stderror::Error>> for KeyValueMemoryState {
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Box<stderror::Error>> {
		Ok(self.storage.get(key).map(|value| value.to_vec()))
	}

	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>) {
		self.storage.insert(key, value);
	}

	fn remove_storage(&mut self, key: &[u8]) {
		self.storage.remove(key);
	}
}

impl AsExternalities<dyn StorageExternalities<Infallible>> for KeyValueMemoryState {
	fn as_externalities(&mut self) -> &mut (dyn StorageExternalities<Infallible> + 'static) {
		self
	}
}

impl AsExternalities<dyn StorageExternalities<Box<stderror::Error>>> for KeyValueMemoryState {
	fn as_externalities(&mut self) -> &mut (dyn StorageExternalities<Box<stderror::Error>> + 'static) {
		self
	}
}

/// Database backed by memory.
pub struct MemoryDatabase<B: Block, A: Auxiliary<B>, S> {
	blocks_and_states: HashMap<B::Identifier, BlockData<B, S>>,
	head: B::Identifier,
	genesis: B::Identifier,
	canon_depth_mappings: HashMap<usize, B::Identifier>,
	auxiliaries: HashMap<A::Key, A>,
}

impl<B: Block, A: Auxiliary<B>, S: Clone> Backend for MemoryDatabase<B, A, S> {
	type Block = B;
	type State = S;
	type Auxiliary = A;
	type Error = Error;
}

impl<B: Block, A: Auxiliary<B>, S: Clone> Database for MemoryDatabase<B, A, S> {
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

impl<B: Block, A: Auxiliary<B>, S: Clone> ChainQuery for MemoryDatabase<B, A, S> {
	fn head(&self) -> B::Identifier {
		self.head
	}

	fn genesis(&self) -> B::Identifier {
		self.genesis
	}

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

/// Memory direct backend.
pub type MemoryBackend<B, A, S> = DirectBackend<MemoryDatabase<B, A, S>>;

impl<B: Block, A: Auxiliary<B>, S: Clone> MemoryLikeBackend for MemoryDatabase<B, A, S> {
	fn new_with_genesis(block: B, genesis_state: S) -> Self {
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
		canon_depth_mappings.insert(0, genesis_id);

		MemoryDatabase {
			blocks_and_states,
			canon_depth_mappings,
			auxiliaries: Default::default(),
			genesis: genesis_id,
			head: genesis_id,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::traits::*;
	use crate::backend::{RwLockBackend, SharedCommittable};
	use std::convert::Infallible;

	#[derive(Clone)]
	pub struct DummyBlock {
		id: usize,
		parent_id: usize,
	}

	impl Block for DummyBlock {
		type Identifier = usize;

		fn id(&self) -> usize { self.id }
		fn parent_id(&self) -> Option<usize> { if self.parent_id == 0 { None } else { Some(self.parent_id) } }
	}

	pub trait CombinedExternalities: NullExternalities + StorageExternalities<Infallible> { }

	impl<T: NullExternalities + StorageExternalities<Infallible>> CombinedExternalities for T { }

	impl<T: CombinedExternalities + 'static> AsExternalities<dyn CombinedExternalities> for T {
		fn as_externalities(&mut self) -> &mut (dyn CombinedExternalities + 'static) {
			self
		}
	}

	pub struct DummyExecutor;

	impl BlockExecutor for DummyExecutor {
		type Error = Error;
		type Block = DummyBlock;
		type Externalities = dyn CombinedExternalities + 'static;

		fn execute_block(
			&self,
			_block: &DummyBlock,
			_state: &mut (dyn CombinedExternalities + 'static),
		) -> Result<(), Error> {
			Ok(())
		}
	}

	#[test]
	fn all_traits_for_importer_are_satisfied() {
		let backend = MemoryBackend::<_, (), KeyValueMemoryState>::new_with_genesis(
			DummyBlock {
				id: 1,
				parent_id: 0,
			},
			Default::default()
		);
		let executor = DummyExecutor;
		let shared = RwLockBackend::new(backend);
		let _ = shared.begin_action(&executor);
	}
}
