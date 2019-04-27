extern crate parity_codec as codec;
extern crate parity_codec_derive as codec_derive;

mod runtime;

use blockchain::backend::MemoryBackend;
use blockchain::chain::{SharedBackend, BlockBuilder};
use blockchain::traits::Block as BlockT;
use libp2p::{secio, NetworkBehaviour};
use libp2p::floodsub::{Floodsub, Topic, TopicBuilder};
use libp2p::kad::Kademlia;
use libp2p::core::swarm::NetworkBehaviourEventProcess;
use futures::{Async, sink::Sink, stream::Stream, sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded}};
use codec::{Encode, Decode};
use codec_derive::{Encode, Decode};
use std::thread;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_io::{AsyncRead, AsyncWrite};
use primitive_types::H256;
use crate::runtime::{Block, Executor, Extrinsic};

#[derive(NetworkBehaviour)]
struct CounterBehaviour<TSubstream: AsyncRead + AsyncWrite> {
	floodsub: Floodsub<TSubstream>,
	kademlia: Kademlia<TSubstream>,

	#[behaviour(ignore)]
	sender: Option<UnboundedSender<Block>>,
	#[behaviour(ignore)]
	backend: SharedBackend<Block, (), MemoryBackend<Block, ()>>,
	#[behaviour(ignore)]
	topic: Topic,
	#[behaviour(ignore)]
	pending_transactions: Option<Arc<Mutex<Vec<Extrinsic>>>>
}

#[derive(Debug, Encode, Decode)]
enum Message {
	Request(H256),
	Block(Block),
	Extrinsic(Extrinsic),
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<libp2p::floodsub::FloodsubEvent> for CounterBehaviour<TSubstream> {
	fn inject_event(&mut self, floodsub_message: libp2p::floodsub::FloodsubEvent) {
		if let libp2p::floodsub::FloodsubEvent::Message(floodsub_message) = floodsub_message {
			let message = Message::decode(&mut &floodsub_message.data[..]).unwrap();
			println!("Received: {:?} from {:?}", message, floodsub_message.source);

			match message {
				Message::Request(hash) => {
					let block = self.backend.block_at(&hash).unwrap();
					self.floodsub.publish(&self.topic, Message::Block(block).encode());
				},
				Message::Block(block) => {
					if let Some(sender) = &mut self.sender {
						sender.start_send(block).unwrap();
					}
				},
				Message::Extrinsic(extrinsic) => {
					if let Some(pending_transactions) = &self.pending_transactions {
						pending_transactions.lock().unwrap().push(extrinsic);
					}
				},
			}
		}
	}
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<libp2p::kad::KademliaOut> for CounterBehaviour<TSubstream> {
	fn inject_event(&mut self, message: libp2p::kad::KademliaOut) {
		if let libp2p::kad::KademliaOut::Discovered { peer_id, .. } = message {
			println!("Discovered via Kademlia {:?}", peer_id);
			self.floodsub.add_node_to_partial_view(peer_id);
		}
	}
}

fn main() {
	let (sender, receiver) = unbounded();

	let genesis_block = Block::genesis();
	let backend = SharedBackend::new(
		MemoryBackend::with_genesis(genesis_block.clone(), Default::default())
	);

	if let Some(to_dial) = std::env::args().nth(1) {
		let (request_sender, request_receiver) = unbounded();

		{
			let backend = backend.clone();
			thread::spawn(move || {
				importer_thread(backend, receiver, request_sender);
			});
		}

		start_network(backend, Some(sender), None, Some(request_receiver), None, Some(to_dial));
	} else {
		let pending_transactions = Arc::new(Mutex::new(Default::default()));

		{
			let backend = backend.clone();
			let pending_transactions = pending_transactions.clone();
			thread::spawn(|| {
				builder_thread(backend, sender, pending_transactions);
			});
		}

		start_network(backend, None, Some(receiver), None, Some(pending_transactions), None);
	}
}

fn start_network(backend: SharedBackend<Block, (), MemoryBackend<Block, ()>>, sender: Option<UnboundedSender<Block>>, mut receiver: Option<UnboundedReceiver<Block>>, mut request_receiver: Option<UnboundedReceiver<Message>>, pending_transactions: Option<Arc<Mutex<Vec<Extrinsic>>>>, to_dial: Option<String>) {
	// Create a random PeerId
	let local_key = if to_dial.is_some() {
		secio::SecioKeyPair::ed25519_raw_key(
			[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
		).unwrap()
	} else {
		secio::SecioKeyPair::ed25519_raw_key(
			[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]
		).unwrap()
	};
	let local_peer_id = local_key.to_peer_id();
	println!("Local peer id: {:?}", local_peer_id);

	let transport = libp2p::build_tcp_ws_secio_mplex_yamux(local_key);
	let topic = TopicBuilder::new("blocks").build();

	let mut swarm = {
		let mut behaviour = CounterBehaviour {
			floodsub: Floodsub::new(local_peer_id.clone()),
			kademlia: Kademlia::new(local_peer_id.clone()),

			topic: topic.clone(),
			backend,
			sender,
			pending_transactions,
		};

		assert!(behaviour.floodsub.subscribe(topic.clone()));
		libp2p::Swarm::new(transport, behaviour, local_peer_id)
	};

	// Listen on all interfaces and whatever port the OS assigns
	let addr = libp2p::Swarm::listen_on(&mut swarm, if to_dial.is_none() { "/ip4/0.0.0.0/tcp/37365".parse().unwrap() } else { "/ip4/0.0.0.0/tcp/0".parse().unwrap() }).unwrap();
	println!("Listening on {:?}", addr);

	// Reach out to another node if specified
	if let Some(to_dial) = to_dial {
		let dialing = to_dial.clone();
		match to_dial.parse() {
			Ok(to_dial) => {
				match libp2p::Swarm::dial_addr(&mut swarm, to_dial) {
					Ok(_) => {
						println!("Dialed {:?}", dialing);
						swarm.floodsub.add_node_to_partial_view(
							"QmSVnNf9HwVMT1Y4cK1P6aoJcEZjmoTXpjKBmAABLMnZEk".parse().unwrap()
						);
						swarm.kademlia.add_connected_address(
							&"QmSVnNf9HwVMT1Y4cK1P6aoJcEZjmoTXpjKBmAABLMnZEk".parse().unwrap(),
							dialing.parse().unwrap(),
						);
					},
					Err(e) => println!("Dial {:?} failed: {:?}", dialing, e)
				}
			},
			Err(err) => println!("Failed to parse address to dial: {:?}", err),
		}
	} else {
		swarm.floodsub.add_node_to_partial_view(
			"QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR".parse().unwrap()
		);
	}

	// Kick it off
	tokio::run(futures::future::poll_fn(move || -> Result<_, ()> {
		if let Some(receiver) = &mut receiver {
			loop {
				match receiver.poll().expect("Error while polling channel") {
					Async::Ready(Some(block)) => {
						println!("Broadcasting block {:?} via floodsub", block);
						swarm.floodsub.publish(&topic, Message::Block(block).encode())
					},
					Async::Ready(None) => panic!("Channel closed"),
					Async::NotReady => break,
				};
			}
		}

		if let Some(request_receiver) = &mut request_receiver {
			loop {
				match request_receiver.poll().expect("Error while polling channel") {
					Async::Ready(Some(message)) => {
						println!("Requesting {:?} via floodsub", message);
						swarm.floodsub.publish(&topic, message.encode())
					},
					Async::Ready(None) => panic!("Channel closed"),
					Async::NotReady => break,
				};
			}
		}

		loop {
			match swarm.poll().expect("Error while polling swarm") {
				Async::Ready(Some(_)) => {},
				Async::Ready(None) | Async::NotReady => break,
			}
		}

		Ok(Async::NotReady)
	}));
}

fn builder_thread(backend_build: SharedBackend<Block, (), MemoryBackend<Block, ()>>, sender: UnboundedSender<Block>, pending_transactions: Arc<Mutex<Vec<Extrinsic>>>) {
	loop {
		thread::sleep(Duration::from_secs(5));

		let head = backend_build.head();
		let executor = Executor;
		println!("Building on top of {}", head);

		// Build a block.
		let mut builder = BlockBuilder::new(&backend_build, &executor, &head, ()).unwrap();
		let pending_transactions = {
			let mut locked = pending_transactions.lock().unwrap();
			let ret = locked.clone();
			locked.clear();
			ret
		};

		for extrinsic in pending_transactions {
			println!("Applying extrinsic {:?}", extrinsic);
			builder.apply_extrinsic(extrinsic).unwrap();
		}
		let op = builder.finalize().unwrap();
		let block = op.block.clone();

		// Import the built block.
		let mut build_importer = backend_build.begin_import(&executor);
		build_importer.import_raw(op);
		build_importer.set_head(block.id());
		build_importer.commit().unwrap();

		sender.unbounded_send(block).unwrap();
	}
}

fn importer_thread(backend_import: SharedBackend<Block, (), MemoryBackend<Block, ()>>, receiver: UnboundedReceiver<Block>, request_sender: UnboundedSender<Message>) {
	let mut receiver = receiver.wait();
	let mut waiting: HashMap<H256, Block> = HashMap::new();
	let mut count = 0;

	loop {
		request_sender.unbounded_send(Message::Extrinsic(Extrinsic::Add(count))).unwrap();
		count += 1;

		let head = backend_import.head();
		let executor = Executor;
		println!("Importing on top of {}", head);

		{
			loop {
				let mut imported = Vec::new();

				for (_, block) in &waiting {
					if backend_import.contains(&block.parent_id().unwrap()).unwrap() {
						let mut importer = backend_import.begin_import(&executor);
						importer.import_block(block.clone()).unwrap();
						importer.set_head(block.id());
						importer.commit().unwrap();
						imported.push(block.id());
					}
				}

				for hash in &imported {
					waiting.remove(hash);
				}

				if imported.len() == 0 {
					break
				}
			}
		}

		let block = receiver.next().unwrap().unwrap();

		// Import the block again to importer.
		let mut importer = backend_import.begin_import(&executor);
		if !backend_import.contains(&block.parent_id().unwrap()).unwrap() {
			request_sender.unbounded_send(Message::Request(block.parent_id().unwrap())).unwrap();
			waiting.insert(block.id(), block);

			continue
		}
		importer.import_block(block.clone()).unwrap();
		importer.set_head(block.id());
		importer.commit().unwrap();
	}
}
