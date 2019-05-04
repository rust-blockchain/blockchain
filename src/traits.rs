//! Common trait definitions related to block context.
//!
//! The consensus layer for this crate works on different levels.
//!
//! On execute level, one uses `ExecuteContext`, and the type parameters defined
//! allows it to be used by an executor -- given a block, and an externalities,
//! it executes the state. This is suitable for plain state transition.
//!
//! On importer level, one uses `ImportContext`. This allows the block to be
//! written into a backend together with auxiliaries. This is suitable for
//! implementing common consensus algorithms.
//!
//! On builder level, one uses `BuildContext`. This allows to be used with
//! `BuilderExecutor` which is able to create new blocks. Used together with
//! `ImportContext`, one can build a proposer.

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
pub trait StorageExternalities {
	/// Read storage value.
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Box<std::error::Error>>;
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
pub trait Backend<B: Block, A: Auxiliary<B>> {
	/// State type
	type State;
	/// Error type
	type Error: stderror::Error + 'static;

	/// Commit operation.
	fn commit(
		&mut self,
		operation: Operation<B, Self::State, A>,
	) -> Result<(), Self::Error>;
}

/// Chain query interface for a backend.
pub trait ChainQuery<B: Block, A: Auxiliary<B>>: Backend<B, A> {
	/// Get the genesis hash of the chain.
	fn genesis(&self) -> B::Identifier;
	/// Get the head of the chain.
	fn head(&self) -> B::Identifier;

	/// Check whether a hash is contained in the chain.
	fn contains(
		&self,
		hash: &B::Identifier,
	) -> Result<bool, <Self as Backend<B, A>>::Error>;

	/// Check whether a block is canonical.
	fn is_canon(
		&self,
		hash: &B::Identifier,
	) -> Result<bool, Self::Error>;

	/// Look up a canonical block via its depth.
	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<B::Identifier>, <Self as Backend<B, A>>::Error>;

	/// Get the auxiliary value by key.
	fn auxiliary(
		&self,
		key: &A::Key,
	) -> Result<Option<A>, <Self as Backend<B, A>>::Error>;

	/// Get the depth of a block.
	fn depth_at(
		&self,
		hash: &B::Identifier,
	) -> Result<usize, <Self as Backend<B, A>>::Error>;

	/// Get children of a block.
	fn children_at(
		&self,
		hash: &B::Identifier,
	) -> Result<Vec<B::Identifier>, <Self as Backend<B, A>>::Error>;

	/// Get the state object of a block.
	fn state_at(
		&self,
		hash: &B::Identifier,
	) -> Result<<Self as Backend<B, A>>::State, <Self as Backend<B, A>>::Error>;

	/// Get the object of a block.
	fn block_at(
		&self,
		hash: &B::Identifier,
	) -> Result<B, <Self as Backend<B, A>>::Error>;
}

/// Trait used for committing block, usually built on top of a backend.
pub trait ImportBlock<B: Block> {
	/// Error type
	type Error: stderror::Error + 'static;

	/// Commit a block into the backend, and handle consensus and auxiliary.
	fn import_block(&mut self, block: B) -> Result<(), Self::Error>;
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
pub trait BuilderExecutor {
	/// Error type
	type Error: stderror::Error + 'static;
	/// Block type
	type Block: Block;
	/// Build block type
	type BuildBlock;
	/// Externalities type
	type Externalities: ?Sized;
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
