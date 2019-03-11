use primitive_types::H256;
use blockchain::traits::{
	Block as BlockT, BlockExecutor, BlockContext, ExtrinsicContext,
	BuilderExecutor, StorageExternalities, ExternalitiesOf,
	BlockOf, ExtrinsicOf, Taggable, Infallible
};
use codec::{Encode, Decode};
use codec_derive::{Decode, Encode};
use sha3::{Digest, Sha3_256};

#[derive(Clone, Debug, Encode, Decode)]
pub struct Block {
	id: H256,
	parent_id: Option<H256>,
	extrinsics: Vec<Extrinsic>,
}

impl Block {
	pub fn calculate_hash(&self) -> H256 {
		let data = (self.parent_id.clone(), self.extrinsics.clone()).encode();
		H256::from_slice(Sha3_256::digest(&data).as_slice())
	}

	pub fn verify_hash(&self) -> bool {
		self.id == self.calculate_hash()
	}

	pub fn fix_hash(&mut self) {
		self.id = self.calculate_hash();
	}

	pub fn genesis() -> Self {
		let mut block = Block {
			id: H256::default(),
			parent_id: None,
			extrinsics: Vec::new(),
		};
		block.fix_hash();
		block
	}
}

impl BlockT for Block {
	type Identifier = H256;

	fn parent_id(&self) -> Option<H256> {
		self.parent_id
	}

	fn id(&self) -> H256 {
		self.id
	}
}

impl Taggable for Block {
	type Tag = Infallible;
}

pub struct Context;

impl BlockContext for Context {
	type Block = Block;
	type Externalities = dyn StorageExternalities + 'static;
	type Auxiliary = ();
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum Extrinsic {
	Add(u128),
}

impl ExtrinsicContext for Context {
	type Extrinsic = Extrinsic;
}

#[derive(Debug)]
pub enum Error {
	Backend(Box<std::error::Error>),
	HashMismatch,
	StateCorruption,
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			Error::HashMismatch => "Hash mismatch".fmt(f)?,
			Error::StateCorruption => "State is corrupted".fmt(f)?,
			Error::Backend(_) => "Backend error".fmt(f)?,
		}

		Ok(())
	}
}

impl std::error::Error for Error { }

#[derive(Clone)]
pub struct Executor;

impl Executor {
	fn read_counter(&self, state: &mut ExternalitiesOf<Context>) -> Result<u128, Error> {
		Ok(
			match state.read_storage(b"counter").map_err(|e| Error::Backend(e))? {
				Some(counter) => {
					u128::decode(&mut counter.as_slice()).ok_or(Error::StateCorruption)?
				},
				None => 0,
			}
		)
	}

	fn write_counter(&self, counter: u128, state: &mut ExternalitiesOf<Context>) {
		state.write_storage(b"counter".to_vec(), counter.encode());
	}
}

impl BlockExecutor<Context> for Executor {
	type Error = Error;

	fn execute_block(
		&self,
		block: &Block,
		state: &mut ExternalitiesOf<Context>,
	) -> Result<(), Error> {
		if !block.verify_hash() {
			return Err(Error::HashMismatch);
		}

		let mut counter = self.read_counter(state)?;

		for extrinsic in &block.extrinsics {
			match extrinsic {
				Extrinsic::Add(add) => counter += add,
			}
		}

		self.write_counter(counter, state);

		Ok(())
	}
}

impl BuilderExecutor<Context> for Executor {
	type Error = Error;

	fn initialize_block(
		&self,
		block: &mut BlockOf<Context>,
		_state: &mut ExternalitiesOf<Context>,
	) -> Result<(), Self::Error> {
		block.parent_id = Some(block.id);
		block.fix_hash();

		Ok(())
	}

	fn apply_extrinsic(
		&self,
		block: &mut BlockOf<Context>,
		extrinsic: ExtrinsicOf<Context>,
		state: &mut ExternalitiesOf<Context>,
	) -> Result<(), Self::Error> {
		let mut counter = self.read_counter(state)?;

		match extrinsic {
			Extrinsic::Add(add) => {
				counter += add;
			},
		}

		self.write_counter(counter, state);
		block.fix_hash();

		Ok(())
	}

	fn finalize_block(
		&self,
		block: &mut BlockOf<Context>,
		_state: &mut ExternalitiesOf<Context>,
	) -> Result<(), Self::Error> {
		block.fix_hash();

		Ok(())
	}
}
