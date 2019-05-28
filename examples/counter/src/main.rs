extern crate parity_codec as codec;
extern crate parity_codec_derive as codec_derive;

mod runtime;

use blockchain::backend::{RwLockBackend, MemoryBackend, KeyValueMemoryState, MemoryLikeBackend, SharedCommittable};
use blockchain::traits::{Block as BlockT, ChainQuery, ImportOperation, SimpleBuilderExecutor, AsExternalities};
use blockchain_network_simple::{BestDepthImporter, BestDepthStatusProducer};
use std::thread;
use std::collections::HashMap;
use clap::{App, SubCommand, AppSettings, Arg};
use crate::runtime::{Block, Executor};

fn main() {
	let matches = App::new("Blockchain counter demo")
		.setting(AppSettings::SubcommandRequiredElseHelp)
		.subcommand(SubCommand::with_name("local")
					.about("Start a local test network"))
		.subcommand(SubCommand::with_name("libp2p")
					.about("Start a libp2p instance")
					.arg(Arg::with_name("port")
						 .short("p")
						 .long("port")
						 .takes_value(true)
						 .help("Port to listen on"))
					.arg(Arg::with_name("author")
						 .long("author")
						 .help("Whether to author blocks")))
		.get_matches();

	if let Some(_) = matches.subcommand_matches("local") {
		local_sync();
		return
	}

	if let Some(matches) = matches.subcommand_matches("libp2p") {
		let port = matches.value_of("port").unwrap_or("37365");
		let author = matches.is_present("author");
		libp2p_sync(port, author);
		return
	}
}

fn local_sync() {
	let genesis_block = Block::genesis();
	let backend_build = RwLockBackend::new(
		MemoryBackend::<_, (), KeyValueMemoryState>::new_with_genesis(
			genesis_block.clone(),
			Default::default()
		)
	);
	let mut peers = HashMap::new();
	for peer_id in 0..4 {
		let backend = if peer_id == 0 {
			backend_build.clone()
		} else {
			RwLockBackend::new(
				MemoryBackend::<_, (), KeyValueMemoryState>::new_with_genesis(
					genesis_block.clone(),
					Default::default()
				)
			)
		};
		let importer = BestDepthImporter::new(Executor, backend.clone());
		let status = BestDepthStatusProducer::new(backend.clone());
		peers.insert(peer_id, (backend, importer, status));
	}
	thread::spawn(move || {
		builder_thread(backend_build);
	});

	blockchain_network_simple::local::start_local_simple_sync(peers);
}

fn libp2p_sync(port: &str, author: bool) {
	let genesis_block = Block::genesis();
	let backend = RwLockBackend::new(
		MemoryBackend::<_, (), KeyValueMemoryState>::new_with_genesis(
			genesis_block.clone(),
			Default::default()
		)
	);
	let importer = BestDepthImporter::new(Executor, backend.clone());
	let status = BestDepthStatusProducer::new(backend.clone());
	if author {
		let backend_build = backend.clone();
		thread::spawn(move || {
			builder_thread(backend_build);
		});
	}
	blockchain_network_simple::libp2p::start_network_simple_sync(port, backend, importer, status);
}


fn builder_thread(backend_build: RwLockBackend<MemoryBackend<Block, (), KeyValueMemoryState>>) {
	loop {
		let head = backend_build.head();
		let executor = Executor;
		println!("Building on top of {}", head);

		// Build a block.
		let parent_block = backend_build.block_at(&head).unwrap();
		let mut pending_state = backend_build.state_at(&head).unwrap();

		let mut unsealed_block = executor.initialize_block(
			&parent_block, pending_state.as_externalities(), ()
		).unwrap();
		executor.finalize_block(
			&mut unsealed_block, pending_state.as_externalities(),
		).unwrap();

		let block = unsealed_block.seal();

		// Import the built block.
		let mut build_importer = backend_build.begin_action(&executor);
		let new_block_hash = block.id();
		let op = ImportOperation { block, state: pending_state };
		build_importer.import_raw(op);
		build_importer.set_head(new_block_hash);
		build_importer.commit().unwrap();
	}
}
