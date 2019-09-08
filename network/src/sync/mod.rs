use core::marker::PhantomData;
use core::task::{Context, Poll, Waker};
use core::pin::Pin;
use core::hash::Hash;
use core::mem;
use core::time::Duration;
use std::collections::{HashMap, VecDeque};
use blockchain::Block;
use blockchain::import::BlockImporter;
use futures::{Stream, StreamExt};
use futures_timer::Interval;
use log::warn;
use rand::seq::IteratorRandom;

#[derive(Default)]
pub struct PeerStatus<H> {
	head_status: Option<(H, usize)>,
	pending_request: Option<usize>,
}

#[derive(PartialEq, Eq)]
pub enum SyncEvent<P> {
	QueryStatus,
	QueryPeerStatus(P),
	QueryBlocks(P),
}

#[derive(PartialEq, Eq)]
pub struct SyncConfig {
	peer_update_frequency: usize,
	update_frequency: usize,
	request_timeout: usize,
}

pub struct NetworkSync<B, P, H, I> {
	head_status: (H, usize),
	tick: usize,
	peers: HashMap<P, PeerStatus<H>>,
	pending_blocks: Vec<B>,
	importer: I,
	waker: Option<Waker>,
	timer: Interval,
	pending_events: VecDeque<SyncEvent<P>>,
	config: SyncConfig,
	_marker: PhantomData<B>,
}

impl<B, P, H, I> NetworkSync<B, P, H, I> where
	P: PartialEq + Eq + Hash,
	H: PartialOrd + Default,
{
	pub fn new(head: H, importer: I, tick_duration: Duration, config: SyncConfig) -> Self {
		Self {
			head_status: (head, 0),
			tick: 0,
			peers: HashMap::new(),
			pending_blocks: Vec::new(),
			importer,
			waker: None,
			timer: Interval::new(tick_duration),
			pending_events: VecDeque::new(),
			config,
			_marker: PhantomData,
		}
	}

	pub fn note_blocks(&mut self, mut blocks: Vec<B>, _source: Option<P>) {
		self.pending_blocks.append(&mut blocks);
		self.wake();
	}

	pub fn note_peer_status(&mut self, peer: P, status: H) {
		self.peers.entry(peer)
			.or_insert(Default::default())
			.head_status = Some((status, self.tick));
	}

	pub fn note_status(&mut self, status: H) {
		self.head_status = (status, self.tick);
	}

	pub fn note_connected(&mut self, peer: P) {
		self.peers.insert(peer, Default::default());
	}

	pub fn note_disconnected(&mut self, peer: P) {
		self.peers.remove(&peer);
	}

	pub fn is_syncing(&self) -> bool {
		for (_, peer_status) in &self.peers {
			if let Some(peer_head_status) = peer_status.head_status.as_ref() {
				if peer_head_status.0 > self.head_status.0 {
					return true
				}
			}
		}
		false
	}

	fn wake(&mut self) {
		if let Some(waker) = self.waker.take() {
			waker.wake()
		}
	}

	fn push_event(&mut self, event: SyncEvent<P>) {
		if !self.pending_events.contains(&event) {
			self.pending_events.push_back(event);
		}
	}
}

impl<B, P, H, I> Stream for NetworkSync<B, P, H, I> where
	P: PartialEq + Eq + Hash + Clone + Unpin,
	H: PartialOrd + Default + Unpin,
	B: Block + Unpin,
	I: BlockImporter<Block=B> + Unpin,
{
	type Item = SyncEvent<P>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		self.waker = Some(cx.waker().clone());

		let mut pending_blocks = Vec::new();
		mem::swap(&mut self.pending_blocks, &mut pending_blocks);
		for block in pending_blocks {
			match self.importer.import_block(block) {
				Ok(()) => (),
				Err(_) => {
					warn!("Error happened on block response message");
				},
			}
		}

		loop {
			match self.timer.poll_next_unpin(cx) {
				Poll::Pending => break,
				Poll::Ready(Some(())) => {
					self.tick += 1;
				},
				Poll::Ready(None) => {
					return Poll::Ready(None)
				},
			}
		}

		let mut new_events = Vec::new();

		if self.tick - self.head_status.1 >= self.config.update_frequency {
			new_events.push(SyncEvent::QueryStatus);
		}

		for (peer, status) in &self.peers {
			if status.head_status.as_ref()
				.map(|h| self.tick - h.1 >= self.config.peer_update_frequency)
				.unwrap_or(false)
			{
				new_events.push(SyncEvent::QueryPeerStatus(peer.clone()));
			}
		}

		if self.is_syncing() {
			let mut need_initialize_new_request = true;

			for (_, status) in &self.peers {
				if let Some(last_tick) = status.pending_request {
					if self.tick - last_tick < self.config.request_timeout {
						need_initialize_new_request = false;
					}
				}
			}

			if need_initialize_new_request {
				let current_tick = self.tick;
				if let Some((peer, status)) = self.peers.iter_mut().choose(&mut rand::thread_rng()) {
					new_events.push(SyncEvent::QueryBlocks(peer.clone()));
					status.pending_request = Some(current_tick);
				}
			}
		}

		for event in new_events {
			self.push_event(event);
		}

		if let Some(event) = self.pending_events.pop_front() {
			Poll::Ready(Some(event))
		} else {
			Poll::Pending
		}
	}
}
