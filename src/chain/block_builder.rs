use super::{Error, Operation, ImportOperation};
use crate::traits::{
	BaseContext, ExtrinsicContext, Backend, BuilderExecutor,
	BlockOf, HashOf, AsExternalities, ExtrinsicOf,
};

pub struct BlockBuilder<C: BaseContext, B: Backend<C>, E> {
	executor: E,
	pending_block: BlockOf<C>,
	pending_state: B::State,
}

impl<C: ExtrinsicContext, B, E> BlockBuilder<C, B, E> where
	B: Backend<C, Operation=Operation<C, B>>,
	E: BuilderExecutor<C>,
{
	pub fn new(backend: &B, executor: E, parent_hash: &HashOf<C>) -> Result<Self, Error> {
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

	pub fn apply_extrinsic(&mut self, extrinsic: ExtrinsicOf<C>) -> Result<(), Error> {
		self.executor.apply_extrinsic(
			&mut self.pending_block,
			extrinsic,
			self.pending_state.as_externalities()
		).map_err(|e| Error::Executor(Box::new(e)))
	}

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
