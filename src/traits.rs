use std::error as stderror;
use std::hash;

pub trait Block: Clone {
	type Hash: Copy + Eq + hash::Hash;

	fn hash(&self) -> Self::Hash;
	fn parent_hash(&self) -> Option<Self::Hash>;
}

pub type ExternalitiesOf<C> = <C as BaseContext>::Externalities;
pub type BlockOf<C> = <C as BaseContext>::Block;
pub type HashOf<C> = <BlockOf<C> as Block>::Hash;
pub type ExtrinsicOf<C> = <C as ExtrinsicContext>::Extrinsic;
pub type AuxiliaryKeyOf<C> = <AuxiliaryOf<C> as Keyed>::Key;
pub type AuxiliaryOf<C> = <C as AuxiliaryContext>::Auxiliary;
pub type TagOf<C> = <C as AuxiliaryContext>::Tag;

pub trait BaseContext {
	type Block: Block;
	type Externalities: ?Sized;
}

pub trait ExtrinsicContext: BaseContext {
	type Extrinsic;
}

pub trait AuxiliaryContext: BaseContext {
	type Tag: Copy + Eq + hash::Hash;
	type Auxiliary: Keyed + Clone;

	fn tags(_block: &Self::Block) -> Vec<Self::Tag> {
		Vec::new()
	}
}

pub trait Keyed {
	type Key: Eq + hash::Hash;

	fn key(&self) -> Self::Key;
}

impl Keyed for () {
	type Key = ();

	fn key(&self) -> () { () }
}

pub trait AsExternalities<E: ?Sized> {
	fn as_externalities(&mut self) -> &mut E;
}

pub trait NullExternalities { }

pub trait StorageExternalities {
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Box<std::error::Error>>;
	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>);
	fn remove_storage(&mut self, key: &[u8]);
}

pub trait Backend<C: AuxiliaryContext>: Sized {
	type State: AsExternalities<ExternalitiesOf<C>>;
	type Operation;
	type Error: stderror::Error + 'static;

	fn genesis(&self) -> HashOf<C>;
	fn head(&self) -> HashOf<C>;

	fn contains(
		&self,
		hash: &HashOf<C>,
	) -> Result<bool, Self::Error>;

	fn is_canon(
		&self,
		hash: &HashOf<C>,
	) -> Result<bool, Self::Error>;

	fn lookup_canon_depth(
		&self,
		depth: usize,
	) -> Result<Option<HashOf<C>>, Self::Error>;

	fn lookup_canon_tag(
		&self,
		key: &TagOf<C>,
	) -> Result<Option<HashOf<C>>, Self::Error>;

	fn auxiliary(
		&self,
		key: &AuxiliaryKeyOf<C>,
	) -> Result<Option<AuxiliaryOf<C>>, Self::Error>;

	fn depth_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<usize, Self::Error>;

	fn children_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Vec<HashOf<C>>, Self::Error>;

	fn state_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<Self::State, Self::Error>;

	fn block_at(
		&self,
		hash: &HashOf<C>,
	) -> Result<BlockOf<C>, Self::Error>;

	fn commit(
		&mut self,
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
