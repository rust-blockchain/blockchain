use parity_codec::{Encode, Decode};
use blockchain::{Block, Auxiliary, AsExternalities, BlockExecutor};
use blockchain::backend::{SharedCommittable, Operation, Store, ImportLock, ChainQuery};
use blockchain::import::{ImportAction, BlockImporter};
use core::cmp::Ordering;
use super::StatusProducer;

#[derive(Eq, Clone, Encode, Decode, Debug)]
pub struct BestDepthStatus {
	pub best_depth: u64,
}

impl Ord for BestDepthStatus {
	fn cmp(&self, other: &Self) -> Ordering {
		self.best_depth.cmp(&other.best_depth)
	}
}

impl PartialOrd for BestDepthStatus {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl PartialEq for BestDepthStatus {
	fn eq(&self, other: &Self) -> bool {
		self == other
	}
}

pub struct BestDepthStatusProducer<Ba> {
	backend: Ba,
}

impl<Ba> BestDepthStatusProducer<Ba> {
	pub fn new(backend: Ba) -> Self {
		Self { backend }
	}
}

impl<Ba: ChainQuery> StatusProducer for BestDepthStatusProducer<Ba> {
	type Status = BestDepthStatus;

	fn generate(&self) -> BestDepthStatus {
		let best_depth = {
			let best_hash = self.backend.head();
			self.backend.depth_at(&best_hash)
				.expect("Best block depth hash cannot fail")
		};

		BestDepthStatus { best_depth: best_depth as u64 }
	}
}

#[derive(Debug)]
pub enum BestDepthError {
	Backend(Box<dyn std::error::Error>),
	Executor(Box<dyn std::error::Error>),
}

impl std::fmt::Display for BestDepthError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{:?}", self)
    }
}

impl std::error::Error for BestDepthError { }

pub struct BestDepthImporter<E, Ba> {
	backend: Ba,
	import_lock: ImportLock,
	executor: E,
}

impl<E: BlockExecutor, Ba: ChainQuery + Store<Block=E::Block>> BestDepthImporter<E, Ba> where
	Ba::Auxiliary: Auxiliary<E::Block>,
	Ba::State: AsExternalities<E::Externalities>,
{
	pub fn new(executor: E, backend: Ba, import_lock: ImportLock) -> Self {
		Self { backend, executor, import_lock }
	}
}

impl<E: BlockExecutor, Ba: ChainQuery + Store<Block=E::Block>> BlockImporter for BestDepthImporter<E, Ba> where
	Ba::Auxiliary: Auxiliary<E::Block>,
	Ba::State: AsExternalities<E::Externalities>,
	Ba: SharedCommittable<Operation=Operation<E::Block, <Ba as Store>::State, <Ba as Store>::Auxiliary>>,
{
	type Block = E::Block;
	type Error = BestDepthError;

	fn import_block(&mut self, block: Ba::Block) -> Result<(), Self::Error> {
		let mut importer = ImportAction::new(
			&self.backend,
			self.import_lock.lock()
		);
		let new_hash = block.id();
		let (current_best_depth, current_best_state, new_depth) = {
			let backend = importer.backend();
			let current_best_hash = backend.head();
			let current_best_depth = backend.depth_at(&current_best_hash)
				.expect("Best block depth hash cannot fail");
			let current_best_state = backend.state_at(&current_best_hash)
				.expect("Best block depth state cannot fail");
			let new_parent_depth = block.parent_id()
				.map(|parent_hash| {
					backend.depth_at(&parent_hash).unwrap()
				})
				.unwrap_or(0);
			(current_best_depth, current_best_state, new_parent_depth + 1)
		};

		let mut pending_state = current_best_state;
		self.executor.execute_block(&block, pending_state.as_externalities())
			.map_err(|e| BestDepthError::Executor(Box::new(e)))?;
		importer.import_block(block, pending_state);
		if new_depth > current_best_depth {
			importer.set_head(new_hash);
		}
		importer.commit().map_err(|e| BestDepthError::Backend(Box::new(e)))?;

		Ok(())
	}
}
