use crate::traits::{Block, Auxiliary, Backend};

macro_rules! define_database {
	( $name:ident, ( $($t:tt)+ ), ( $($p:tt)+ ) ) => {
		/// A database trait where the reference to database is unique.
		pub trait $name: $($p)+ {
			/// Insert a new block into the database.
			fn insert_block(
				$($t)+,
				id: <Self::Block as Block>::Identifier,
				block: Self::Block,
				state: Self::State,
				depth: usize,
				children: Vec<<Self::Block as Block>::Identifier>,
				is_canon: bool
			);
			/// Push a new child to a block.
			fn push_child(
				$($t)+,
				id: <Self::Block as Block>::Identifier,
				child: <Self::Block as Block>::Identifier,
			);
			/// Set canon.
			fn set_canon(
				$($t)+,
				id: <Self::Block as Block>::Identifier,
				is_canon: bool
			);
			/// Insert canon depth mapping.
			fn insert_canon_depth_mapping(
				$($t)+,
				depth: usize,
				id: <Self::Block as Block>::Identifier,
			);
			/// Remove canon depth mapping.
			fn remove_canon_depth_mapping(
				$($t)+,
				depth: &usize
			);
			/// Insert a new auxiliary.
			fn insert_auxiliary(
				$($t)+,
				key: <Self::Auxiliary as Auxiliary<Self::Block>>::Key,
				value: Self::Auxiliary
			);
			/// Remove an auxiliary.
			fn remove_auxiliary(
				$($t)+,
				key: &<Self::Auxiliary as Auxiliary<Self::Block>>::Key,
			);
			/// Set head.
			fn set_head(
				$($t)+,
				head: <Self::Block as Block>::Identifier
			);
		}
	}
}

define_database!(Database, (&mut self), (Backend));
define_database!(SharedDatabase, (&self), (Backend + Clone));
