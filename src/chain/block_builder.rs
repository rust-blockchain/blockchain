use super::{Error, SharedBackend};
use crate::traits::{
	Backend, BuilderExecutor, AsExternalities,
	ImportOperation, Block, Auxiliary,
};

/// Block builder.
pub struct BlockBuilder<'a, E: BuilderExecutor, A: Auxiliary<E::Block>, Ba: Backend<E::Block, A>> {
	executor: &'a E,
	pending_block: E::Block,
	pending_state: Ba::State,
}

impl<'a, E: BuilderExecutor, A: Auxiliary<E::Block>, Ba: Backend<E::Block, A>> BlockBuilder<'a, E, A, Ba> where
	Ba::State: AsExternalities<E::Externalities>,
{
	/// Create a new block builder.
	pub fn new(backend: &SharedBackend<E::Block, A, Ba>, executor: &'a E, parent_hash: &<E::Block as Block>::Identifier, inherent: E::Inherent) -> Result<Self, Error> {
		let mut pending_block = backend.block_at(parent_hash)
			.map_err(|e| Error::Backend(Box::new(e)))?;

		let mut pending_state = backend.state_at(parent_hash)
			.map_err(|e| Error::Backend(Box::new(e)))?;

		executor.initialize_block(&mut pending_block, pending_state.as_externalities(), inherent)
			.map_err(|e| Error::Executor(Box::new(e)))?;

		Ok(Self {
			executor, pending_block, pending_state,
		})
	}

	/// Apply extrinsic to the block builder.
	pub fn apply_extrinsic(&mut self, extrinsic: E::Extrinsic) -> Result<(), Error> {
		self.executor.apply_extrinsic(
			&mut self.pending_block,
			extrinsic,
			self.pending_state.as_externalities()
		).map_err(|e| Error::Executor(Box::new(e)))
	}

	/// Finalize the block.
	pub fn finalize(mut self) -> Result<ImportOperation<E::Block, Ba::State>, Error> {
		self.executor.finalize_block(
			&mut self.pending_block,
			self.pending_state.as_externalities()
		).map_err(|e| Error::Executor(Box::new(e)))?;

		Ok(ImportOperation {
			block: self.pending_block,
			state: self.pending_state,
		})
	}
}
