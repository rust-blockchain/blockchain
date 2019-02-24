extern crate parity_codec as codec;
extern crate parity_codec_derive as codec_derive;

mod runtime;

use blockchain::backend::MemoryBackend;
use blockchain::chain::{SharedBackend, BlockBuilder};
use blockchain::traits::Block as BlockT;
use crate::runtime::{Block, Executor, Extrinsic};

fn main() {
	let genesis_block = Block::genesis();
	let backend_build = SharedBackend::new(
		MemoryBackend::with_genesis(genesis_block.clone(), Default::default())
	);
	let backend_import = SharedBackend::new(
		MemoryBackend::with_genesis(genesis_block.clone(), Default::default())
	);

	loop {
		let head = backend_build.head();
		let executor = Executor;
		println!("Building on top of {}", head);

		// Build a block.
		let mut builder = BlockBuilder::new(&backend_build, &executor, &head).unwrap();
		builder.apply_extrinsic(Extrinsic::Add(5)).unwrap();
		let op = builder.finalize().unwrap();
		let block = op.block.clone();

		// Import the built block.
		let mut build_importer = backend_build.begin_import(&executor);
		build_importer.import_raw(op);
		build_importer.set_head(*block.hash());
		build_importer.commit().unwrap();

		// Import the block again to importer.
		let mut importer = backend_import.begin_import(&executor);
		importer.import_block(block.clone()).unwrap();
		importer.set_head(*block.hash());
		importer.commit().unwrap();
	}
}
