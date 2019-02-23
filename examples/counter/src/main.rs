extern crate parity_codec as codec;
extern crate parity_codec_derive as codec_derive;

use primitive_types::H256;
use blockchain::traits::{
	Block as BlockT, BlockExecutor, BaseContext, ExtrinsicContext,
	BuilderExecutor, StorageExternalities, ExternalitiesOf,
	BlockOf, ExtrinsicOf, Backend,
};
use blockchain::backend::MemoryBackend;
use blockchain::chain::{Importer, BlockBuilder};
use codec::{Encode, Decode};
use codec_derive::{Decode, Encode};
use sha3::{Digest, Sha3_256};

#[derive(Clone, Debug, Encode, Decode)]
pub struct Block {
	hash: H256,
	parent_hash: Option<H256>,
	extrinsics: Vec<Extrinsic>,
}

impl Block {
	pub fn calculate_hash(&self) -> H256 {
		let data = (self.parent_hash.clone(), self.extrinsics.clone()).encode();
		H256::from_slice(Sha3_256::digest(&data).as_slice())
	}

	pub fn verify_hash(&self) -> bool {
		self.hash == self.calculate_hash()
	}

	pub fn fix_hash(&mut self) {
		self.hash = self.calculate_hash();
	}
}

impl BlockT for Block {
	type Hash = H256;

	fn parent_hash(&self) -> Option<&H256> {
		self.parent_hash.as_ref()
	}

	fn hash(&self) -> &H256 {
		&self.hash
	}
}

pub struct Context;

impl BaseContext for Context {
	type Block = Block;
	type Externalities = dyn StorageExternalities + 'static;
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
		block.parent_hash = Some(block.hash);
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

fn main() {
	let genesis_block = {
		let mut block = Block {
			hash: H256::default(),
			parent_hash: None,
			extrinsics: Vec::new(),
		};
		block.fix_hash();
		block
	};

	let backend_build = MemoryBackend::with_genesis(genesis_block.clone(), Default::default());
	let backend_import = MemoryBackend::with_genesis(genesis_block.clone(), Default::default());
	let mut importer = Importer::new(backend_import.clone(), Executor);
	let mut build_importer = Importer::new(backend_build.clone(), Executor);

	loop {
		let head = backend_build.head();
		println!("Building on top of {}", head);

		// Build a block.
		let mut builder = BlockBuilder::new(&backend_build, Executor, &head).unwrap();
		builder.apply_extrinsic(Extrinsic::Add(5)).unwrap();
		let op = builder.finalize().unwrap();
		let block = op.block.clone();

		// Import the built block.
		build_importer.import_raw(op).unwrap();
		build_importer.set_head(*block.hash()).unwrap();
		backend_build.commit(build_importer.pop().unwrap()).unwrap();

		// Import the block again to importer.
		importer.import_block(block.clone()).unwrap();
		importer.set_head(*block.hash()).unwrap();
		backend_import.commit(importer.pop().unwrap()).unwrap();
	}
}
