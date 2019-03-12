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

/// Externalities of a context.
pub type ExternalitiesOf<C> = <C as ExecuteContext>::Externalities;
/// Block of a context.
pub type BlockOf<C> = <C as ExecuteContext>::Block;
/// Hash of a context.
pub type IdentifierOf<C> = <BlockOf<C> as Block>::Identifier;
/// Extrinsic of a context.
pub type ExtrinsicOf<C> = <C as BuildContext>::Extrinsic;
/// Auxiliary key of a context.
pub type AuxiliaryKeyOf<C> = <AuxiliaryOf<C> as Auxiliary<C>>::Key;
/// Auxiliary of a context.
pub type AuxiliaryOf<C> = <C as ImportContext>::Auxiliary;

/// Context containing all basic information of block execution.
///
/// This is everything needed to build an execution layer for a block.
pub trait ExecuteContext {
	/// Block type
	type Block: Block;
	/// Externalities type
	type Externalities: ?Sized;
}

/// Context containing importer information.
///
/// This is everything needed to build a consensus layer for a block.
pub trait ImportContext: ExecuteContext {
	/// Auxiliary type
	type Auxiliary: Auxiliary<Self>;
}

/// Context allowing block construction via extrinsic.
///
/// This is everything needed to build a proposer layer on top, except an
/// executor.
pub trait BuildContext: ExecuteContext {
	/// Extrinsic type
	type Extrinsic;
}

/// A value where the key is contained in.
pub trait Auxiliary<C: ?Sized + ExecuteContext>: Clone {
	/// Key type
	type Key: Copy + Eq + hash::Hash;

	/// Return the key of this object.
	fn key(&self) -> Self::Key;
	/// Return block ids associated with this auxiliary. If the backend
	/// removes any of the blocks listed here, it is expected to remove
	/// this auxiliary entry, and trigger a recalculation for the
	/// consensus engine.
	fn associated(&self) -> Vec<IdentifierOf<C>> {
		Vec::new()
	}
}

impl<C: ExecuteContext> Auxiliary<C> for () {
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

/// Backend for a block context.
pub trait Backend<C: ImportContext>: Sized {
	/// State type
	type State: AsExternalities<ExternalitiesOf<C>>;
	/// Operation type
	type Operation;
	/// Error type
	type Error: stderror::Error + 'static;

	/// Get the genesis hash of the chain.
	fn genesis(&self) -> IdentifierOf<C>;
	/// Get the head of the chain.
	fn head(&self) -> IdentifierOf<C>;

	/// Check whether a hash is contained in the chain.
	fn contains(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<bool, Self::Error>;

	/// Check whether a block is canonical.
	fn is_canon(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<bool, Self::Error>;

	/// Look up a canonical block via its depth.
	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<IdentifierOf<C>>, Self::Error>;

	/// Get the auxiliary value by key.
	fn auxiliary(
		&self,
		key: &AuxiliaryKeyOf<C>,
	) -> Result<Option<AuxiliaryOf<C>>, Self::Error>;

	/// Get the depth of a block.
	fn depth_at(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<usize, Self::Error>;

	/// Get children of a block.
	fn children_at(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<Vec<IdentifierOf<C>>, Self::Error>;

	/// Get the state object of a block.
	fn state_at(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<Self::State, Self::Error>;

	/// Get the object of a block.
	fn block_at(
		&self,
		hash: &IdentifierOf<C>,
	) -> Result<BlockOf<C>, Self::Error>;

	/// Commit operation.
	fn commit(
		&mut self,
		operation: Self::Operation,
	) -> Result<(), Self::Error>;
}

/// Trait used for committing block, usually built on top of a backend.
pub trait ImportBlock<C: ImportContext> {
	/// Error type
	type Error: stderror::Error + 'static;

	/// Commit a block into the backend, and handle consensus and auxiliary.
	fn import_block(&mut self, block: BlockOf<C>) -> Result<(), Self::Error>;
}

/// Block executor
pub trait BlockExecutor<C: ExecuteContext>: Sized {
	/// Error type
	type Error: stderror::Error + 'static;

	/// Execute the block via a block object and given state.
	fn execute_block(
		&self,
		block: &BlockOf<C>,
		state: &mut ExternalitiesOf<C>
	) -> Result<(), Self::Error>;
}

/// Builder executor
pub trait BuilderExecutor<C: BuildContext>: Sized {
	/// Error type
	type Error: stderror::Error + 'static;

	/// Initialize a block from the parent block, and given state.
	fn initialize_block(
		&self,
		block: &mut BlockOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;

	/// Apply extrinsic to a given block.
	fn apply_extrinsic(
		&self,
		block: &mut BlockOf<C>,
		extrinsic: ExtrinsicOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;

	/// Finalize a block.
	fn finalize_block(
		&self,
		block: &mut BlockOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;
}
