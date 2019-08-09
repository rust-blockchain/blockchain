mod difficulty;
mod depth;

pub use self::depth::{BestDepthStatus, BestDepthStatusProducer, BestDepthError, BestDepthImporter};

use parity_codec::{Encode, Decode};
use blockchain::backend::{ChainQuery, Store, SharedCommittable, ImportLock};
use blockchain::import::BlockImporter;
use core::marker::PhantomData;
use crate::{NetworkHandle, NetworkEnvironment, NetworkEvent};

pub trait StatusProducer {
	type Status: Ord + Encode + Decode;

	fn generate(&self) -> Self::Status;
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum NetworkSyncMessage<B, S> {
	Status(S),
	BlockRequest {
		start_depth: u64,
		count: u64,
	},
	BlockResponse {
		blocks: Vec<B>,
	},
}

pub struct NetworkSync<P, Ba, I, St> {
	backend: Ba,
	import_lock: ImportLock,
	importer: I,
	status: St,
	_marker: PhantomData<P>,
}

impl<P, Ba, I, St> NetworkSync<P, Ba, I, St> {
	pub fn new(backend: Ba, import_lock: ImportLock, importer: I, status: St) -> Self {
		Self {
			backend, import_lock, importer, status,
			_marker: PhantomData,
		}
	}
}

impl<P, Ba: Store, I, St: StatusProducer> NetworkEnvironment for NetworkSync<P, Ba, I, St> {
	type PeerId = P;
	type Message = NetworkSyncMessage<Ba::Block, St::Status>;
}

impl<P, Ba: SharedCommittable + ChainQuery, I: BlockImporter<Block=Ba::Block>, St: StatusProducer> NetworkEvent for NetworkSync<P, Ba, I, St> {
	fn on_tick<H: NetworkHandle>(
		&mut self, handle: &mut H
	) where
		H: NetworkEnvironment<PeerId=Self::PeerId, Message=Self::Message>
	{
		let status = self.status.generate();
		handle.broadcast(NetworkSyncMessage::Status(status));
	}

	fn on_message<H: NetworkHandle>(
		&mut self, handle: &mut H, peer: &P, message: Self::Message
	) where
		H: NetworkEnvironment<PeerId=Self::PeerId, Message=Self::Message>
	{
		match message {
			NetworkSyncMessage::Status(peer_status) => {
				let status = self.status.generate();
				let best_depth = {
					let best_hash = self.backend.head();
					self.backend.depth_at(&best_hash)
						.expect("Best block depth hash cannot fail")
				};

				if peer_status > status {
					handle.send(peer, NetworkSyncMessage::BlockRequest {
						start_depth: best_depth as u64 + 1,
						count: 256,
					});
				}
			},
			NetworkSyncMessage::BlockRequest {
				start_depth,
				count,
			} => {
				let mut ret = Vec::new();
				{
					let _ = self.import_lock.lock();
					for d in start_depth..(start_depth + count) {
						match self.backend.lookup_canon_depth(d as usize) {
							Ok(Some(hash)) => {
								let block = self.backend.block_at(&hash)
									.expect("Found hash cannot fail");
								ret.push(block);
							},
							_ => break,
						}
					}
				}
				handle.send(peer, NetworkSyncMessage::BlockResponse {
					blocks: ret
				});
			},
			NetworkSyncMessage::BlockResponse {
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
