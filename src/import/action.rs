use std::sync::MutexGuard;
use crate::backend::{SharedCommittable, Store, Operation, ImportOperation};
use crate::{Block, Auxiliary};

/// Block importer.
pub struct ImportAction<'a, Ba: Store> {
	backend: &'a Ba,
	pending: Operation<Ba::Block, Ba::State, Ba::Auxiliary>,
	_guard: MutexGuard<'a, ()>,
}

impl<'a, Ba: Store> From<ImportAction<'a, Ba>> for Operation<Ba::Block, Ba::State, Ba::Auxiliary> {
	fn from(
		action: ImportAction<'a, Ba>
	) -> Operation<Ba::Block, Ba::State, Ba::Auxiliary> {
		action.pending
	}
}

impl<'a, Ba: Store> ImportAction<'a, Ba> where
	Ba: SharedCommittable<Operation=Operation<<Ba as Store>::Block, <Ba as Store>::State, <Ba as Store>::Auxiliary>>,
{
	/// Create a new import action.
	pub fn new(backend: &'a Ba, import_guard: MutexGuard<'a, ()>) -> Self {
		Self {
			backend,
			pending: Default::default(),
			_guard: import_guard
		}
	}

	/// Get the associated backend of the importer.
	pub fn backend(&self) -> &'a Ba {
		self.backend
	}

	/// Import a new block.
	pub fn import_block(&mut self, block: Ba::Block, state: Ba::State) {
		self.import_raw(ImportOperation { block, state });
	}

	/// Import a raw operation.
	pub fn import_raw(&mut self, raw: ImportOperation<Ba::Block, Ba::State>) {
		self.pending.import_block.push(raw);
	}

	/// Set head to given hash.
	pub fn set_head(&mut self, head: <Ba::Block as Block>::Identifier) {
		self.pending.set_head = Some(head);
	}

	/// Insert auxiliary value.
	pub fn insert_auxiliary(&mut self, aux: Ba::Auxiliary) {
		self.pending.insert_auxiliaries.push(aux);
	}

	/// Remove auxiliary value.
	pub fn remove_auxiliary(&mut self, aux_key: <Ba::Auxiliary as Auxiliary<Ba::Block>>::Key) {
		self.pending.remove_auxiliaries.push(aux_key);
	}

	/// Commit operation and drop import lock.
	pub fn commit(self) -> Result<(), Ba::Error> {
		Ok(self.backend.commit(self.into())?)
	}
}
