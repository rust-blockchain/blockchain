//! Common trait definitions related to block context.

use std::error as stderror;
use std::hash;

/// A block contains a hash, and reference a parent block via parent hash.
pub trait Block: Clone {
	/// Hash type of the block.
	type Identifier: Copy + Eq + hash::Hash;

	/// Get the block hash.
	fn id(&self) -> Self::Identifier;
	/// Get the parent block hash. None if this block is genesis.
	fn parent_id(&self) -> Option<Self::Identifier>;
}

/// A value where the key is contained in.
pub trait Auxiliary<B: Block>: Clone {
	/// Key type
	type Key: Copy + Eq + hash::Hash;

	/// Return the key of this object.
	fn key(&self) -> Self::Key;
	/// Return block ids associated with this auxiliary. If the backend
	/// removes any of the blocks listed here, it is expected to remove
	/// this auxiliary entry, and trigger a recalculation for the
	/// consensus engine.
	fn associated(&self) -> Vec<B::Identifier> {
		Vec::new()
	}
}

impl<B: Block> Auxiliary<B> for () {
	type Key = ();

	fn key(&self) -> () { () }
}

/// Trait that allows conversion into externalities.
pub trait AsExternalities<E: ?Sized> {
	/// Turn this object into externalities.
	fn as_externalities(&mut self) -> &mut E;
}

/// Null externalities.
pub trait NullExternalities { }

/// Externalities for reading a key value based storage.
pub trait StorageExternalities<Error> {
	/// Read storage value.
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error>;
	/// Write storage value.
	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>);
	/// Remove storage value.
	fn remove_storage(&mut self, key: &[u8]);
}

/// Import operation.
pub struct ImportOperation<B, S> {
	/// Block to be imported.
	pub block: B,
	/// Associated state of the block.
	pub state: S,
}

/// Operation for a backend.
pub struct Operation<B: Block, S, A: Auxiliary<B>> {
	/// Import operation.
	pub import_block: Vec<ImportOperation<B, S>>,
	/// Set head operation.
	pub set_head: Option<B::Identifier>,
	/// Auxiliaries insertion operation.
	pub insert_auxiliaries: Vec<A>,
	/// Auxiliaries removal operation.
	pub remove_auxiliaries: Vec<A::Key>,
}

impl<B: Block, S, A: Auxiliary<B>> Default for Operation<B, S, A> {
	fn default() -> Self {
		Self {
			import_block: Vec::new(),
			set_head: None,
			insert_auxiliaries: Vec::new(),
			remove_auxiliaries: Vec::new(),
		}
	}
}

/// Commit-able backend for a block context.
pub trait Backend {
	/// Block type
	type Block: Block;
	/// State type
	type State;
	/// Auxiliary type
	type Auxiliary: Auxiliary<Self::Block>;
	/// Error type
	type Error: stderror::Error + 'static;

	/// Commit operation.
	fn commit(
		&mut self,
		operation: Operation<Self::Block, Self::State, Self::Auxiliary>,
	) -> Result<(), Self::Error>;
}

/// Chain query interface for a backend.
pub trait ChainQuery: Backend {
	/// Get the genesis hash of the chain.
	fn genesis(&self) -> <Self::Block as Block>::Identifier;
	/// Get the head of the chain.
	fn head(&self) -> <Self::Block as Block>::Identifier;

	/// Check whether a hash is contained in the chain.
	fn contains(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error>;

	/// Check whether a block is canonical.
	fn is_canon(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<bool, Self::Error>;

	/// Look up a canonical block via its depth.
	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<<Self::Block as Block>::Identifier>, Self::Error>;

	/// Get the auxiliary value by key.
	fn auxiliary(
		&self,
		key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
	) -> Result<Option<Self::Auxiliary>, Self::Error>;

	/// Get the depth of a block.
	fn depth_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<usize, Self::Error>;

	/// Get children of a block.
	fn children_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Vec<<Self::Block as Block>::Identifier>, Self::Error>;

	/// Get the state object of a block.
	fn state_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::State, Self::Error>;

	/// Get the object of a block.
	fn block_at(
		&self,
		hash: &<Self::Block as Block>::Identifier,
	) -> Result<Self::Block, Self::Error>;
}

/// Trait used for committing blocks, usually built on top of a backend.
pub trait BlockImporter {
	/// Block type
	type Block: Block;
	/// Error type
	type Error: stderror::Error + 'static;

	/// Commit a block into the backend, and handle consensus and auxiliary.
	fn import_block(&mut self, block: Self::Block) -> Result<(), Self::Error>;
}

/// Trait used for committing prebuilt blocks, usually built on top of a backend.
pub trait RawImporter {
	/// Block type
	type Block: Block;
	/// State type
	type State;
	/// Error type
	type Error: stderror::Error + 'static;

	/// Commit a prebuilt block into the backend, and handle consensus and auxiliary.
	fn import_raw(
		&mut self,
		operation: ImportOperation<Self::Block, Self::State>
	) -> Result<(), Self::Error>;
}

/// Block executor
pub trait BlockExecutor {
	/// Error type
	type Error: stderror::Error + 'static;
	/// Block type
	type Block: Block;
	/// Externalities type
	type Externalities: ?Sized;

	/// Execute the block via a block object and given state.
	fn execute_block(
		&self,
		block: &Self::Block,
		state: &mut Self::Externalities
	) -> Result<(), Self::Error>;
}

/// Builder executor
pub trait SimpleBuilderExecutor: BlockExecutor {
	/// Build block type
	type BuildBlock;
	/// Inherent
	type Inherent;
	/// Extrinsic
	type Extrinsic;

	/// Initialize a block from the parent block, and given state.
	fn initialize_block(
		&self,
		parent_block: &Self::Block,
		state: &mut Self::Externalities,
		inherent: Self::Inherent,
	) -> Result<Self::BuildBlock, Self::Error>;

	/// Apply extrinsic to a given block.
	fn apply_extrinsic(
		&self,
		block: &mut Self::BuildBlock,
		extrinsic: Self::Extrinsic,
		state: &mut Self::Externalities,
	) -> Result<(), Self::Error>;

	/// Finalize a block.
	fn finalize_block(
		&self,
		block: &mut Self::BuildBlock,
		state: &mut Self::Externalities,
	) -> Result<(), Self::Error>;
}
