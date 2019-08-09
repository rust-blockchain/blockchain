#[cfg(feature = "std")]
use std::error as stderror;
use alloc::vec::Vec;
use core::hash;

/// A block contains a hash, and reference a parent block via parent hash.
pub trait Block: Clone {
	/// Hash type of the block.
	type Identifier: Clone + Eq + hash::Hash;

	/// Get the block hash.
	fn id(&self) -> Self::Identifier;
	/// Get the parent block hash. None if this block is genesis.
	fn parent_id(&self) -> Option<Self::Identifier>;
}

/// A value where the key is contained in.
pub trait Auxiliary<B: Block>: Clone {
	/// Key type
	type Key: Clone + Eq + hash::Hash;

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

impl NullExternalities for () { }
impl AsExternalities<dyn NullExternalities> for () {
	fn as_externalities(&mut self) -> &mut (dyn NullExternalities + 'static) {
		self
	}
}

/// Externalities for reading a key value based storage.
pub trait StorageExternalities<Error> {
	/// Read storage value.
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error>;
	/// Write storage value.
	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>);
	/// Remove storage value.
	fn remove_storage(&mut self, key: &[u8]);
}

/// Block executor
pub trait BlockExecutor {
	#[cfg(feature = "std")]
	/// Error type
	type Error: stderror::Error + 'static;
	#[cfg(not(feature = "std"))]
	/// Error type
	type Error: 'static;
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
pub trait ExtrinsicBuilder: BlockExecutor {
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
