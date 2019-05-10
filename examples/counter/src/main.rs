extern crate parity_codec as codec;
extern crate parity_codec_derive as codec_derive;

mod runtime;
mod network;

use blockchain::backend::MemoryBackend;
use blockchain::chain::{SharedBackend, BlockBuilder};
use blockchain::traits::{Block as BlockT, ChainQuery};
use std::thread;
use std::time::Duration;
use std::collections::HashMap;
use clap::{App, SubCommand, AppSettings};
use crate::runtime::{Block, Executor};
use crate::network::{BestDepthImporter};

fn main() {
	let matches = App::new("Blockchain counter demo")
		.setting(AppSettings::SubcommandRequiredElseHelp)
		.subcommand(SubCommand::with_name("local")
					.about("Start a local test network"))
		.get_matches();

	if let Some(_) = matches.subcommand_matches("local") {
		local_sync();
	}
}

fn local_sync() {
	let genesis_block = Block::genesis();
	let backend_build = SharedBackend::new(
		MemoryBackend::<_, ()>::with_genesis(genesis_block.clone(), Default::default())
	);
	let mut peers = HashMap::new();
	for peer_id in 0..4 {
		let backend = if peer_id == 0 {
			backend_build.clone()
		} else {
			SharedBackend::new(
				MemoryBackend::<_, ()>::with_genesis(genesis_block.clone(), Default::default())
			)
		};
		let importer = BestDepthImporter::new(Executor, backend.clone());
		peers.insert(peer_id, (backend, importer));
	}
	thread::spawn(move || {
		builder_thread(backend_build);
	});

	network::local::start_local_best_depth_sync(peers);
}


fn builder_thread(backend_build: SharedBackend<MemoryBackend<Block, ()>>) {
	loop {
		thread::sleep(Duration::from_secs(5));

		let head = backend_build.read().head();
		let executor = Executor;
		println!("Building on top of {}", head);

		// Build a block.
		let builder = BlockBuilder::new(&backend_build, &executor, &head, ()).unwrap();
		let op = builder.finalize().unwrap();
		let block = op.block.clone();

		// Import the built block.
		let mut build_importer = backend_build.begin_import(&executor);
		build_importer.import_raw(op);
		build_importer.set_head(block.id());
		build_importer.commit().unwrap();
	}
}
