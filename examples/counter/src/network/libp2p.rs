use core::fmt::Debug;
use core::hash::Hash;
use core::marker::PhantomData;
use core::time::Duration;
use core::ops::DerefMut;
use codec::{Encode, Decode};
use libp2p::{secio, identity, NetworkBehaviour, PeerId};
use libp2p::mdns::Mdns;
use libp2p::floodsub::{Floodsub, Topic, TopicBuilder};
use libp2p::kad::Kademlia;
use libp2p::core::swarm::NetworkBehaviourEventProcess;
use futures::{Async, sink::Sink, stream::Stream, sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded}};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_timer::Interval;
use blockchain::chain::SharedBackend;
use blockchain::traits::{ImportBlock, ChainQuery, Backend};
use crate::network::{BestDepthMessage, BestDepthSync, NetworkEnvironment, NetworkHandle, NetworkEvent};

#[derive(NetworkBehaviour)]
struct Behaviour<TSubstream: AsyncRead + AsyncWrite, Ba: Backend, I> {
	floodsub: Floodsub<TSubstream>,
	kademlia: Kademlia<TSubstream>,
	mdns: Mdns<TSubstream>,

	#[behaviour(ignore)]
	sync: Option<BestDepthSync<PeerId, Ba, I>>,
	#[behaviour(ignore)]
	topic: Topic,
}

impl<TSubstream: AsyncRead + AsyncWrite, Ba: Backend, I> NetworkEnvironment for Behaviour<TSubstream, Ba, I> {
	type PeerId = PeerId;
	type Message = BestDepthMessage<Ba::Block>;
}

impl<TSubstream: AsyncRead + AsyncWrite, Ba: Backend, I> NetworkHandle for Behaviour<TSubstream, Ba, I>  where
	Ba::Block: Encode,
{
	fn send(&mut self, _peer: &PeerId, message: BestDepthMessage<Ba::Block>) {
		self.floodsub.publish(&self.topic, message.encode());
	}

	fn broadcast(&mut self, message: BestDepthMessage<Ba::Block>) {
		self.floodsub.publish(&self.topic, message.encode());
	}
}

impl<TSubstream: AsyncRead + AsyncWrite, Ba: Backend, I> NetworkBehaviourEventProcess<libp2p::floodsub::FloodsubEvent> for Behaviour<TSubstream, Ba, I> where
	I: ImportBlock<Block=Ba::Block>,
	Ba: ChainQuery,
	Ba::Block: Encode + Decode + Debug
{
	fn inject_event(&mut self, floodsub_message: libp2p::floodsub::FloodsubEvent) {
		if let libp2p::floodsub::FloodsubEvent::Message(floodsub_message) = floodsub_message {
			let message = BestDepthMessage::<Ba::Block>::decode(&mut &floodsub_message.data[..]).unwrap();
			println!("Received: {:?} from {:?}", message, floodsub_message.source);

			let mut sync = self.sync.take().expect("Sync is initialized to Some");
			sync.on_message(self, &floodsub_message.source, message);
			self.sync.replace(sync);
		}
	}
}


impl<TSubstream: AsyncRead + AsyncWrite, Ba: Backend, I> NetworkBehaviourEventProcess<libp2p::kad::KademliaOut> for Behaviour<TSubstream, Ba, I> {
	fn inject_event(&mut self, message: libp2p::kad::KademliaOut) {
		if let libp2p::kad::KademliaOut::Discovered { peer_id, .. } = message {
			println!("Discovered via Kademlia {:?}", peer_id);
			self.floodsub.add_node_to_partial_view(peer_id);
		}
	}
}

impl<TSubstream: AsyncRead + AsyncWrite, Ba: Backend, I> NetworkBehaviourEventProcess<libp2p::mdns::MdnsEvent> for Behaviour<TSubstream, Ba, I> {
    fn inject_event(&mut self, event: libp2p::mdns::MdnsEvent) {
        match event {
            libp2p::mdns::MdnsEvent::Discovered(list) => {
                for (peer, _) in list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            },
            libp2p::mdns::MdnsEvent::Expired(list) => {
                for (peer, _) in list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                    }
                }
            }
        }
    }
}

pub fn start_network_best_depth_sync<Ba, I>(
	port: &str,
	backend: SharedBackend<Ba>,
	importer: I,
) where
	Ba: ChainQuery + Send + Sync + 'static,
	Ba::Block: Debug + Encode + Decode + Send + Sync,
	I: ImportBlock<Block=Ba::Block> + Send + Sync + 'static,
{
    // Create a random PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
	println!("Local peer id: {:?}", local_peer_id);

	let transport = libp2p::build_tcp_ws_secio_mplex_yamux(local_key);
	let topic = TopicBuilder::new("blocks").build();

	let mut sync = BestDepthSync {
		backend, importer,
		_marker: PhantomData,
	};

	let mut swarm = {
		let mut behaviour = Behaviour {
			floodsub: Floodsub::new(local_peer_id.clone()),
			kademlia: Kademlia::new(local_peer_id.clone()),
			mdns: libp2p::mdns::Mdns::new().expect("Failed to create mDNS service"),

			sync: Some(sync),
			topic: topic.clone(),
		};

		assert!(behaviour.floodsub.subscribe(topic.clone()));
		libp2p::Swarm::new(transport, behaviour, local_peer_id)
	};

	// Listen on all interfaces and whatever port the OS assigns
	let addr = libp2p::Swarm::listen_on(&mut swarm, format!("/ip4/0.0.0.0/tcp/{}", port).parse().unwrap()).unwrap();
	println!("Listening on {:?}", addr);

	let mut interval = Interval::new_interval(Duration::new(5, 0));
	let mut listening = false;
    tokio::run(futures::future::poll_fn(move || -> Result<_, ()> {
        loop {
            match interval.poll().expect("Error while polling interval") {
                Async::Ready(Some(_)) => {
					let behaviour = swarm.deref_mut();
					let mut sync = behaviour.sync.take().unwrap();
					sync.on_tick(behaviour);
					behaviour.sync.replace(sync);
				},
                Async::Ready(None) => panic!("Interval closed"),
                Async::NotReady => break,
            };
        }

        loop {
            match swarm.poll().expect("Error while polling swarm") {
                Async::Ready(Some(_)) => (),
                Async::Ready(None) | Async::NotReady => {
                    if !listening {
                        if let Some(a) = libp2p::Swarm::listeners(&swarm).next() {
                            println!("Listening on {:?}", a);
                            listening = true;
                        }
                    }
                    break
                }
            }
        }

        Ok(Async::NotReady)
	}));
}
