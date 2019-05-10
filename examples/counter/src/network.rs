use std::collections::HashMap;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::sync::{Arc, mpsc::{SyncSender, Receiver, sync_channel}};
use core::marker::PhantomData;
use core::hash::Hash;
use core::fmt::Debug;
use codec_derive::{Encode, Decode};
use blockchain::chain::SharedBackend;
use blockchain::traits::{Backend, ChainQuery, ImportBlock, BlockExecutor, Auxiliary, AsExternalities, Block as BlockT};
use crate::runtime::Block;

pub trait NetworkEnvironment {
	type PeerId;
	type Message;
}

pub trait NetworkHandle: NetworkEnvironment {
	fn send(&mut self, peer: &Self::PeerId, message: Self::Message);
	fn broadcast(&mut self, message: Self::Message);
}

pub trait NetworkEvent: NetworkEnvironment {
	fn on_tick<H: NetworkHandle>(&mut self, _handle: &mut H) where
		H: NetworkEnvironment<PeerId=Self::PeerId, Message=Self::Message> { }
	fn on_message<H: NetworkHandle>(
		&mut self, _handle: &mut H, _peer: &Self::PeerId, _message: Self::Message
	) where H: NetworkEnvironment<PeerId=Self::PeerId, Message=Self::Message> { }
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum BestDepthMessage<B> {
	Status {
		best_depth: usize,
	},
	BlockRequest {
		start_depth: usize,
		count: usize,
	},
	BlockResponse {
		blocks: Vec<B>,
	},
}

pub struct BestDepthSync<P, Ba: Backend, I> {
	backend: SharedBackend<Ba>,
	importer: I,
	_marker: PhantomData<P>,
}

impl<P, Ba: Backend, I> NetworkEnvironment for BestDepthSync<P, Ba, I> {
	type PeerId = P;
	type Message = BestDepthMessage<Ba::Block>;
}

impl<P, Ba: ChainQuery, I: ImportBlock<Block=Ba::Block>> NetworkEvent for BestDepthSync<P, Ba, I> {
	fn on_tick<H: NetworkHandle>(
		&mut self, handle: &mut H
	) where
		H: NetworkEnvironment<PeerId=Self::PeerId, Message=Self::Message>
	{
		let best_depth = {
			let backend = self.backend.read();
			let best_hash = backend.head();
			backend.depth_at(&best_hash)
				.expect("Best block depth hash cannot fail")
		};

		handle.broadcast(BestDepthMessage::Status { best_depth });
	}

	fn on_message<H: NetworkHandle>(
		&mut self, handle: &mut H, peer: &P, message: BestDepthMessage<Ba::Block>
	) where
		H: NetworkEnvironment<PeerId=Self::PeerId, Message=Self::Message>
	{
		match message {
			BestDepthMessage::Status {
				best_depth: peer_best_depth
			} => {
				let best_depth = {
					let backend = self.backend.read();
					let best_hash = backend.head();
					backend.depth_at(&best_hash)
						.expect("Best block depth hash cannot fail")
				};

				if peer_best_depth > best_depth {
					handle.send(peer, BestDepthMessage::BlockRequest {
						start_depth: best_depth + 1,
						count: peer_best_depth - best_depth,
					});
				}
			},
			BestDepthMessage::BlockRequest {
				start_depth,
				count,
			} => {
				let mut ret = Vec::new();
				{
					let backend = self.backend.read();
					for d in start_depth..(start_depth + count) {
						match backend.lookup_canon_depth(d) {
							Ok(Some(hash)) => {
								let block = backend.block_at(&hash)
									.expect("Found hash cannot fail");
								ret.push(block);
							},
							_ => {
								println!("warn: error happened on block request message");
								break
							},
						}
					}
				}
				handle.send(peer, BestDepthMessage::BlockResponse {
					blocks: ret
				});
			},
			BestDepthMessage::BlockResponse {
				blocks,
			} => {
				for block in blocks {
					match self.importer.import_block(block) {
						Ok(()) => (),
						Err(_) => {
							println!("warn: error happened on block response message");
							break
						},
					}
				}
			},
		}
	}
}

pub struct BestDepthImporter<E: BlockExecutor, Ba: Backend<Block=E::Block>> where
	Ba::Auxiliary: Auxiliary<E::Block>,
{
	backend: SharedBackend<Ba>,
	executor: E,
}

impl<E: BlockExecutor, Ba: ChainQuery + Backend<Block=E::Block>> BestDepthImporter<E, Ba> where
	Ba::Auxiliary: Auxiliary<E::Block>,
	Ba::State: AsExternalities<E::Externalities>,
{
	pub fn new(executor: E, backend: SharedBackend<Ba>) -> Self {
		Self { backend, executor }
	}
}

impl<E: BlockExecutor, Ba: ChainQuery + Backend<Block=E::Block>> ImportBlock for BestDepthImporter<E, Ba> where
	Ba::Auxiliary: Auxiliary<E::Block>,
	Ba::State: AsExternalities<E::Externalities>,
{
	type Block = E::Block;
	type Error = Ba::Error;

	fn import_block(&mut self, block: Ba::Block) -> Result<(), Ba::Error> {
		let mut importer = self.backend.begin_import(&self.executor);
		let new_hash = block.id();
		let (current_best_hash, current_best_depth, new_depth) = {
			let backend = importer.backend().read();
			let current_best_hash = backend.head();
			let current_best_depth = backend.depth_at(&current_best_hash)
				.expect("Best block depth hash cannot fail");
			let new_parent_depth = block.parent_id()
				.map(|parent_hash| {
					backend.depth_at(&parent_hash).unwrap()
				})
				.unwrap_or(0);
			(current_best_hash, current_best_depth, new_parent_depth + 1)
		};

		importer.import_block(block).unwrap();
		if new_depth > current_best_depth {
			importer.set_head(new_hash);
		}
		importer.commit().unwrap();

		Ok(())
	}
}

pub struct LocalNetwork<P, B> {
	senders: HashMap<P, SyncSender<(P, BestDepthMessage<B>)>>,
}

impl<P: Eq + Hash + Clone, B: Clone> LocalNetwork<P, B> {
	pub fn send(&self, peer: &P, message: (P, BestDepthMessage<B>)) {
		self.senders.get(peer).unwrap()
			.send(message).unwrap();
	}

	pub fn broadcast(&self, message: (P, BestDepthMessage<B>)) {
		for sender in self.senders.values() {
			sender.send(message.clone()).unwrap();
		}
	}
}

#[derive(Clone)]
pub struct LocalNetworkHandle<P, B> {
	peer_id: P,
	network: Arc<LocalNetwork<P, B>>
}

impl<P, B> NetworkEnvironment for LocalNetworkHandle<P, B> {
	type PeerId = P;
	type Message = BestDepthMessage<B>;
}

impl<P: Eq + Hash + Clone, B: Clone> NetworkHandle for LocalNetworkHandle<P, B> {
	fn send(&mut self, peer: &P, message: BestDepthMessage<B>) {
		self.network.send(peer, (self.peer_id.clone(), message));
	}

	fn broadcast(&mut self, message: BestDepthMessage<B>) {
		self.network.broadcast((self.peer_id.clone(), message));
	}
}

pub fn start_local_best_depth_peer<P, Ba, I>(
	mut handle: LocalNetworkHandle<P, Ba::Block>,
	receiver: Receiver<(P, BestDepthMessage<Ba::Block>)>,
	peer_id: P,
	backend: SharedBackend<Ba>,
	importer: I,
) -> JoinHandle<()> where
	P: Debug + Eq + Hash + Clone + Send + Sync + 'static,
	Ba: ChainQuery + Send + Sync + 'static,
	Ba::Block: Debug + Send + Sync,
	I: ImportBlock<Block=Ba::Block> + Send + Sync + 'static,
{
	thread::spawn(move || {
		let this_peer_id = peer_id.clone();

		let mut sync = BestDepthSync {
			backend, importer,
			_marker: PhantomData
		};

		loop {
			for (peer_id, message) in receiver.try_iter() {
				println!("peer[{:?}] on message {:?}", this_peer_id, message);
				sync.on_message(&mut handle, &peer_id, message);
			}

			thread::sleep(Duration::from_millis(1000));
			println!("peer[{:?}] on tick", this_peer_id);
			sync.on_tick(&mut handle);
		}
	})
}

pub fn start_local_best_depth_sync<P, Ba, I>(
	peers: HashMap<P, (SharedBackend<Ba>, I)>
) where
	P: Debug + Eq + Hash + Clone + Send + Sync + 'static,
	Ba: ChainQuery + Send + Sync + 'static,
	Ba::Block: Debug + Send + Sync,
	I: ImportBlock<Block=Ba::Block> + Send + Sync + 'static,
{
	let mut senders: HashMap<P, SyncSender<(P, BestDepthMessage<Ba::Block>)>> = HashMap::new();
	let mut peers_with_receivers: HashMap<P, (SharedBackend<Ba>, I, Receiver<(P, BestDepthMessage<Ba::Block>)>)> = HashMap::new();
	for (peer_id, (backend, importer)) in peers {
		let (sender, receiver) = sync_channel(10);
		senders.insert(peer_id.clone(), sender);
		peers_with_receivers.insert(peer_id, (backend, importer, receiver));
	}

	let mut join_handles: Vec<JoinHandle<()>> = Vec::new();
	let network = Arc::new(LocalNetwork { senders });
	for (peer_id, (backend, importer, receiver)) in peers_with_receivers {
		let join_handle = start_local_best_depth_peer(
			LocalNetworkHandle {
				peer_id: peer_id.clone(),
				network: network.clone(),
			},
			receiver,
			peer_id,
			backend,
			importer,
		);
		join_handles.push(join_handle);
	}

	for join_handle in join_handles {
		join_handle.join().unwrap();
	}
}
