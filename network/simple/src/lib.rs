extern crate parity_codec as codec;

pub mod local;
pub mod libp2p;

use core::marker::PhantomData;
use core::cmp::Ordering;
use codec::{Encode, Decode};
use blockchain::chain::SharedBackend;
use blockchain::traits::{Backend, ChainQuery, ImportBlock, BlockExecutor, Auxiliary, AsExternalities, Block as BlockT};

pub trait StatusProducer {
	type Status: Ord + Encode + Decode;

	fn generate(&self) -> Self::Status;
}

#[derive(Eq, Clone, Encode, Decode, Debug)]
pub struct BestDepthStatus {
	pub best_depth: usize,
}

impl Ord for BestDepthStatus {
	fn cmp(&self, other: &Self) -> Ordering {
		self.best_depth.cmp(&other.best_depth)
	}
}

impl PartialOrd for BestDepthStatus {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl PartialEq for BestDepthStatus {
	fn eq(&self, other: &Self) -> bool {
		self == other
	}
}

pub struct BestDepthStatusProducer<Ba: Backend> {
	backend: SharedBackend<Ba>,
}

impl<Ba: Backend> BestDepthStatusProducer<Ba> {
	pub fn new(backend: SharedBackend<Ba>) -> Self {
		Self { backend }
	}
}

impl<Ba: ChainQuery> StatusProducer for BestDepthStatusProducer<Ba> {
	type Status = BestDepthStatus;

	fn generate(&self) -> BestDepthStatus {
		let best_depth = {
			let backend = self.backend.read();
			let best_hash = backend.head();
			backend.depth_at(&best_hash)
				.expect("Best block depth hash cannot fail")
		};

		BestDepthStatus { best_depth }
	}
}

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
pub enum SimpleSyncMessage<B, S> {
	Status(S),
	BlockRequest {
		start_depth: usize,
		count: usize,
	},
	BlockResponse {
		blocks: Vec<B>,
	},
}

pub struct SimpleSync<P, Ba: Backend, I, St> {
	backend: SharedBackend<Ba>,
	importer: I,
	status: St,
	_marker: PhantomData<P>,
}

impl<P, Ba: Backend, I, St: StatusProducer> NetworkEnvironment for SimpleSync<P, Ba, I, St> {
	type PeerId = P;
	type Message = SimpleSyncMessage<Ba::Block, St::Status>;
}

impl<P, Ba: ChainQuery, I: ImportBlock<Block=Ba::Block>, St: StatusProducer> NetworkEvent for SimpleSync<P, Ba, I, St> {
	fn on_tick<H: NetworkHandle>(
		&mut self, handle: &mut H
	) where
		H: NetworkEnvironment<PeerId=Self::PeerId, Message=Self::Message>
	{
		let status = self.status.generate();
		handle.broadcast(SimpleSyncMessage::Status(status));
	}

	fn on_message<H: NetworkHandle>(
		&mut self, handle: &mut H, peer: &P, message: Self::Message
	) where
		H: NetworkEnvironment<PeerId=Self::PeerId, Message=Self::Message>
	{
		match message {
			SimpleSyncMessage::Status(peer_status) => {
				let status = self.status.generate();
				let best_depth = {
					let backend = self.backend.read();
					let best_hash = backend.head();
					backend.depth_at(&best_hash)
						.expect("Best block depth hash cannot fail")
				};

				if peer_status > status {
					handle.send(peer, SimpleSyncMessage::BlockRequest {
						start_depth: best_depth + 1,
						count: 256,
					});
				}
			},
			SimpleSyncMessage::BlockRequest {
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
							_ => break,
						}
					}
				}
				handle.send(peer, SimpleSyncMessage::BlockResponse {
					blocks: ret
				});
			},
			SimpleSyncMessage::BlockResponse {
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
	blockchain::chain::Error: From<E::Error> + From<Ba::Error>,
{
	type Block = E::Block;
	type Error = blockchain::chain::Error;

	fn import_block(&mut self, block: Ba::Block) -> Result<(), Self::Error> {
		let mut importer = self.backend.begin_import(&self.executor);
		let new_hash = block.id();
		let (current_best_depth, new_depth) = {
			let backend = importer.backend().read();
			let current_best_hash = backend.head();
			let current_best_depth = backend.depth_at(&current_best_hash)
				.expect("Best block depth hash cannot fail");
			let new_parent_depth = block.parent_id()
				.map(|parent_hash| {
					backend.depth_at(&parent_hash).unwrap()
				})
				.unwrap_or(0);
			(current_best_depth, new_parent_depth + 1)
		};

		importer.import_block(block)?;
		if new_depth > current_best_depth {
			importer.set_head(new_hash);
		}
		importer.commit()?;

		Ok(())
	}
}
