use std::error as stderror;
use crate::{Block, Auxiliary};

/// Backend store definition for a block context.
pub trait Store {
	/// Block type
	type Block: Block;
	/// State type
	type State;
	/// Auxiliary type
	type Auxiliary: Auxiliary<Self::Block>;
	/// Error type
	type Error: stderror::Error + 'static;
}

/// Backend operation error.
pub trait OperationError: stderror::Error {
	/// Invalid operation
	fn invalid_operation() -> Self;
	/// Trying to import a block that is genesis
	fn block_is_genesis() -> Self;
}

/// Chain query interface for a backend.
pub trait ChainQuery: Store {
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

/// Database settlement for chain backend.
pub trait ChainSettlement: Store {
	/// Insert a new block into the database.
	fn insert_block(
		&mut self,
		id: <Self::Block as Block>::Identifier,
		block: Self::Block,
		state: Self::State,
		depth: usize,
		children: Vec<<Self::Block as Block>::Identifier>,
		is_canon: bool
	);
	/// Push a new child to a block.
	fn push_child(
		&mut self,
		id: <Self::Block as Block>::Identifier,
		child: <Self::Block as Block>::Identifier,
	);
	/// Set canon.
	fn set_canon(
		&mut self,
		id: <Self::Block as Block>::Identifier,
		is_canon: bool
	);
	/// Insert canon depth mapping.
	fn insert_canon_depth_mapping(
		&mut self,
		depth: usize,
		id: <Self::Block as Block>::Identifier,
	);
	/// Remove canon depth mapping.
	fn remove_canon_depth_mapping(
		&mut self,
		depth: &usize
	);
	/// Insert a new auxiliary.
	fn insert_auxiliary(
		&mut self,
		key: <Self::Auxiliary as Auxiliary<Self::Block>>::Key,
		value: Self::Auxiliary
	);
	/// Remove an auxiliary.
	fn remove_auxiliary(
		&mut self,
		key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
	);
	/// Set head.
	fn set_head(
		&mut self,
		head: <Self::Block as Block>::Identifier
	);
}

/// Committable backend.
pub trait Committable: Store {
	/// Operation type for commit.
	type Operation;

	/// Commit operation.
	fn commit(
		&mut self,
		operation: Self::Operation,
	) -> Result<(), Self::Error>;
}

/// Shared committable backend.
pub trait SharedCommittable: Store + Clone {
	/// Operation type for commit.
	type Operation;

	/// Commit operation.
	fn commit(
		&self,
		operation: Self::Operation,
	) -> Result<(), Self::Error>;
}
