use std::error as stderror;
use std::fmt;
use std::hash;

pub trait Block: Clone {
	type Hash: Copy + Eq + fmt::Debug + hash::Hash;

	fn hash(&self) -> &Self::Hash;
	fn parent_hash(&self) -> Option<&Self::Hash>;
}

pub type ExternalitiesOf<C> = <C as BaseContext>::Externalities;
pub type BlockOf<C> = <C as BaseContext>::Block;
pub type HashOf<C> = <BlockOf<C> as Block>::Hash;
pub type ExtrinsicOf<C> = <C as ExtrinsicContext>::Extrinsic;

pub trait BaseContext {
	type Block: Block;
	type Externalities: ?Sized;
}

pub trait ExtrinsicContext: BaseContext {
	type Extrinsic;
}

pub trait AsExternalities<E: ?Sized> {
	fn as_externalities(&mut self) -> &mut E;
}

pub trait NullExternalities { }

pub trait StorageExternalities {
	fn read_storage(&self, key: &[u8]) -> Option<Vec<u8>>;
	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>);
	fn remove_storage(&mut self, key: &[u8]);
}

pub trait Backend<C: BaseContext>: Sized {
	type State: AsExternalities<ExternalitiesOf<C>>;
	type Operation;
	type Error: stderror::Error + 'static;

	fn head(&self) -> HashOf<C>;

	fn depth_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Option<usize>, Self::Error>;

	fn state_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Option<Self::State>, Self::Error>;

	fn block_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Option<BlockOf<C>>, Self::Error>;

	fn commit(
		&self,
		operation: Self::Operation,
	) -> Result<(), Self::Error>;
}

pub trait CommitBlock<C: BaseContext> {
	type Error: stderror::Error + 'static;

	fn commit_block(&mut self, block: BlockOf<C>) -> Result<(), Self::Error>;
}

pub trait BlockExecutor<C: BaseContext>: Sized {
	type Error: stderror::Error + 'static;

	fn execute_block(
		&self,
		block: &BlockOf<C>,
		state: &mut ExternalitiesOf<C>
	) -> Result<(), Self::Error>;
}

pub trait BuilderExecutor<C: ExtrinsicContext>: Sized {
	type Error: stderror::Error + 'static;

	fn initialize_block(
		&self,
		block: &mut BlockOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;

	fn apply_extrinsic(
		&self,
		block: &mut BlockOf<C>,
		extrinsic: ExtrinsicOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;

	fn finalize_block(
		&self,
		block: &mut BlockOf<C>,
		state: &mut ExternalitiesOf<C>,
	) -> Result<(), Self::Error>;
}
