use super::{Error, SharedBackend};
use crate::backend::{Operation, ImportOperation};
use crate::traits::{
	ExtrinsicContext, Backend, BuilderExecutor,
	BlockOf, IdentifierOf, AsExternalities, ExtrinsicOf,
	BuilderExecutorOf,
};

/// Block builder.
pub struct BlockBuilder<'a, C: ExtrinsicContext, B: Backend<C>> {
	executor: &'a BuilderExecutorOf<C>,
	pending_block: BlockOf<C>,
	pending_state: B::State,
}

impl<'a, C: ExtrinsicContext, B> BlockBuilder<'a, C, B> where
	B: Backend<C, Operation=Operation<C, B>>,
{
	/// Create a new block builder.
	pub fn new(backend: &SharedBackend<C, B>, executor: &'a BuilderExecutorOf<C>, parent_hash: &IdentifierOf<C>) -> Result<Self, Error> {
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
	pub fn apply_extrinsic(&mut self, extrinsic: ExtrinsicOf<C>) -> Result<(), Error> {
		self.executor.apply_extrinsic(
			&mut self.pending_block,
			extrinsic,
			self.pending_state.as_externalities()
		).map_err(|e| Error::Executor(Box::new(e)))
	}

	/// Finalize the block.
	pub fn finalize(mut self) -> Result<ImportOperation<C, B>, Error> {
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
