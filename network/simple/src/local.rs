use std::collections::HashMap;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::sync::{Arc, mpsc::{SyncSender, Receiver, sync_channel}};
use core::marker::PhantomData;
use core::hash::Hash;
use core::fmt::Debug;
use blockchain::chain::SharedBackend;
use blockchain::traits::{ChainQuery, ImportBlock};
use crate::{BestDepthSync, BestDepthMessage, NetworkEnvironment, NetworkHandle, NetworkEvent};

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
