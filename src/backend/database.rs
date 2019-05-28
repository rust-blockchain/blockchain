use crate::traits::{Block, Auxiliary, Backend};

/// A database trait where the reference to database is unique.
pub trait Database: Backend {
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
