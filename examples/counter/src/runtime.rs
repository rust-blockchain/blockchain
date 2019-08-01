use primitive_types::H256;
use blockchain::{
	Block as BlockT, BlockExecutor,
	SimpleBuilderExecutor, StorageExternalities,
};
use codec::{Encode, Decode};
use sha3::{Digest, Sha3_256};
use std::convert::Infallible;

const DIFFICULTY: usize = 2;

fn is_all_zero(arr: &[u8]) -> bool {
	arr.iter().all(|i| *i == 0)
}

#[derive(Clone, Debug)]
pub struct UnsealedBlock {
	parent_hash: Option<H256>,
	extrinsics: Vec<Extrinsic>,
}

impl UnsealedBlock {
	pub fn seal(self) -> Block {
		let mut block = Block {
			parent_hash: self.parent_hash,
			extrinsics: self.extrinsics,
			nonce: 0,
		};

		while !is_all_zero(&block.id()[0..DIFFICULTY]) {
			block.nonce += 1;
		}

		block
	}
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct Block {
	parent_hash: Option<H256>,
	extrinsics: Vec<Extrinsic>,
	nonce: u64,
}

impl Block {
	pub fn genesis() -> Self {
		Block {
			parent_hash: None,
			extrinsics: Vec::new(),
			nonce: 0,
		}
	}
}

impl BlockT for Block {
	type Identifier = H256;

	fn parent_id(&self) -> Option<H256> {
		self.parent_hash
	}

	fn id(&self) -> H256 {
		H256::from_slice(Sha3_256::digest(&self.encode()).as_slice())
	}
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum Extrinsic {
	Add(u128),
}

#[derive(Debug)]
pub enum Error {
	Backend(Box<dyn std::error::Error>),
	DifficultyTooLow,
	StateCorruption,
}

impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl std::error::Error for Error { }

impl From<Error> for blockchain::import::Error {
	fn from(error: Error) -> Self {
		blockchain::import::Error::Executor(Box::new(error))
	}
}

#[derive(Clone)]
pub struct Executor;

impl Executor {
	fn read_counter(&self, state: &mut <Self as BlockExecutor>::Externalities) -> Result<u128, Error> {
		Ok(
			match state.read_storage(b"counter").expect("Error is infallible; qed") {
				Some(counter) => {
					u128::decode(&mut counter.as_slice()).ok_or(Error::StateCorruption)?
				},
				None => 0,
			}
		)
	}

	fn write_counter(&self, counter: u128, state: &mut <Self as BlockExecutor>::Externalities) {
		state.write_storage(b"counter".to_vec(), counter.encode());
	}
}

impl BlockExecutor for Executor {
	type Error = Error;
	type Block = Block;
	type Externalities = dyn StorageExternalities<Infallible> + 'static;

	fn execute_block(
		&self,
		block: &Self::Block,
		state: &mut Self::Externalities,
	) -> Result<(), Error> {
		if !is_all_zero(&block.id()[0..DIFFICULTY]) {
			return Err(Error::DifficultyTooLow);
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

impl SimpleBuilderExecutor for Executor {
	type BuildBlock = UnsealedBlock;
	type Extrinsic = Extrinsic;
	type Inherent = ();

	fn initialize_block(
		&self,
		block: &Self::Block,
		_state: &mut Self::Externalities,
		_inherent: (),
	) -> Result<Self::BuildBlock, Self::Error> {
		Ok(UnsealedBlock {
			parent_hash: Some(block.id()),
			extrinsics: Vec::new(),
		})
	}

	fn apply_extrinsic(
		&self,
		block: &mut Self::BuildBlock,
		extrinsic: Self::Extrinsic,
		state: &mut Self::Externalities,
	) -> Result<(), Self::Error> {
		let mut counter = self.read_counter(state)?;

		match &extrinsic {
			Extrinsic::Add(add) => {
				counter += add;
			},
		}

		self.write_counter(counter, state);
		block.extrinsics.push(extrinsic);

		Ok(())
	}

	fn finalize_block(
		&self,
		_block: &mut Self::BuildBlock,
		_state: &mut Self::Externalities,
	) -> Result<(), Self::Error> {
		Ok(())
	}
}
