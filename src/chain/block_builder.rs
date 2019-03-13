use super::{Error, SharedBackend};
use crate::backend::{Operation, ImportOperation};
use crate::traits::{
	Backend, BuilderExecutor,
	BlockOf, IdentifierOf, AsExternalities,
	ImportContext,
};

/// Block builder.
pub struct BlockBuilder<'a, E: BuilderExecutor, B: Backend<E::Context>> where
	E::Context: ImportContext
{
	executor: &'a E,
	pending_block: BlockOf<E::Context>,
	pending_state: B::State,
}

impl<'a, E: BuilderExecutor, B> BlockBuilder<'a, E, B> where
	B: Backend<E::Context, Operation=Operation<E::Context, B>>,
	E::Context: ImportContext,
{
	/// Create a new block builder.
	pub fn new(backend: &SharedBackend<E::Context, B>, executor: &'a E, parent_hash: &IdentifierOf<E::Context>) -> Result<Self, Error> {
		let mut pending_block = backend.block_at(parent_hash)
			.map_err(|e| Error::Backend(Box::new(e)))?;

		let mut pending_state = backend.state_at(parent_hash)
			.map_err(|e| Error::Backend(Box::new(e)))?;

		executor.initialize_block(&mut pending_block, pending_state.as_externalities())
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
	pub fn finalize(mut self) -> Result<ImportOperation<E::Context, B>, Error> {
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
