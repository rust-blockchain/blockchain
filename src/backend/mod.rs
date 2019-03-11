//! Basic backend definitions and memory backend.

mod memory;
mod route;

pub use self::memory::{MemoryState, MemoryBackend};
pub use self::route::{tree_route, TreeRoute};

use crate::traits::{Backend, BlockContext, BlockOf, IdentifierOf, AuxiliaryOf, AuxiliaryKeyOf};

/// Import operation.
pub struct ImportOperation<C: BlockContext, B: Backend<C>> {
	/// Block to be imported.
	pub block: BlockOf<C>,
	/// Associated state of the block.
	pub state: B::State,
}

/// Operation for a backend.
pub struct Operation<C: BlockContext, B: Backend<C>> {
	/// Import operation.
	pub import_block: Vec<ImportOperation<C, B>>,
	/// Set head operation.
	pub set_head: Option<IdentifierOf<C>>,
	/// Auxiliaries insertion operation.
	pub insert_auxiliaries: Vec<AuxiliaryOf<C>>,
	/// Auxiliaries removal operation.
	pub remove_auxiliaries: Vec<AuxiliaryKeyOf<C>>,
}

impl<C: BlockContext, B> Default for Operation<C, B> where
	B: Backend<C>
{
	fn default() -> Self {
		Self {
			import_block: Vec::new(),
			set_head: None,
			insert_auxiliaries: Vec::new(),
			remove_auxiliaries: Vec::new(),
		}
	}
}
