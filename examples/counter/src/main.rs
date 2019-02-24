extern crate parity_codec as codec;
extern crate parity_codec_derive as codec_derive;

mod runtime;

use blockchain::backend::MemoryBackend;
use blockchain::chain::{SharedBackend, BlockBuilder};
use blockchain::traits::Block as BlockT;
use libp2p::{secio, NetworkBehaviour};
use libp2p::floodsub::{Floodsub, TopicBuilder};
use libp2p::kad::Kademlia;
use libp2p::core::swarm::NetworkBehaviourEventProcess;
use futures::{Async, sink::Sink, stream::Stream, sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded}};
use codec::{Encode, Decode};
use std::thread;
use std::time::Duration;
use tokio_io::{AsyncRead, AsyncWrite};
use crate::runtime::{Block, Executor, Extrinsic};

#[derive(NetworkBehaviour)]
struct CounterBehaviour<TSubstream: AsyncRead + AsyncWrite> {
	floodsub: Floodsub<TSubstream>,
	kademlia: Kademlia<TSubstream>,

	#[behaviour(ignore)]
	sender: Option<UnboundedSender<Block>>,
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<libp2p::floodsub::FloodsubEvent> for CounterBehaviour<TSubstream> {
	fn inject_event(&mut self, message: libp2p::floodsub::FloodsubEvent) {
		if let libp2p::floodsub::FloodsubEvent::Message(message) = message {
			let block = Block::decode(&mut &message.data[..]).unwrap();
			println!("Received: {:?} from {:?}", block, message.source);

			if let Some(sender) = &mut self.sender {
				sender.start_send(block).unwrap();
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

	if let Some(to_dial) = std::env::args().nth(1) {
		thread::spawn(move || {
			importer_thread(receiver);
		});

		start_network(Some(sender), None, Some(to_dial));
	} else {
		thread::spawn(move || {
			builder_thread(sender);
		});

		start_network(None, Some(receiver), None);
	}
}

fn start_network(sender: Option<UnboundedSender<Block>>, mut receiver: Option<UnboundedReceiver<Block>>, to_dial: Option<String>) {
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
	let new_block_topic = TopicBuilder::new("chat").build();

	let mut swarm = {
		let mut behaviour = CounterBehaviour {
			floodsub: Floodsub::new(local_peer_id.clone()),
			kademlia: Kademlia::new(local_peer_id.clone()),

			sender,
		};

		assert!(behaviour.floodsub.subscribe(new_block_topic.clone()));
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
	}

	// Kick it off
	tokio::run(futures::future::poll_fn(move || -> Result<_, ()> {
		if let Some(receiver) = &mut receiver {
			loop {
				match receiver.poll().expect("Error while polling channel") {
					Async::Ready(Some(block)) => {
						println!("Broadcasting block {:?} via floodsub", block);
						swarm.floodsub.publish(&new_block_topic, block.encode())
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

fn builder_thread(sender: UnboundedSender<Block>) {
	let genesis_block = Block::genesis();
	let backend_build = SharedBackend::new(
		MemoryBackend::with_genesis(genesis_block.clone(), Default::default())
	);

	loop {
		thread::sleep(Duration::from_secs(5));

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

		sender.unbounded_send(block).unwrap();
	}
}

fn importer_thread(receiver: UnboundedReceiver<Block>) {
	let genesis_block = Block::genesis();
	let backend_import = SharedBackend::new(
		MemoryBackend::with_genesis(genesis_block.clone(), Default::default())
	);
	let mut receiver = receiver.wait();

	loop {
		let head = backend_import.head();
		let executor = Executor;
		println!("Importing on top of {}", head);

		let block = receiver.next().unwrap().unwrap();

		// Import the block again to importer.
		let mut importer = backend_import.begin_import(&executor);
		importer.import_block(block.clone()).unwrap();
		importer.set_head(*block.hash());
		importer.commit().unwrap();
	}
}
