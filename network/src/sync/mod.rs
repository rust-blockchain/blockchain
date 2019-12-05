use core::task::{Context, Poll, Waker};
use core::pin::Pin;
use core::hash::Hash;
use core::mem;
use core::time::Duration;
use std::collections::{HashMap, VecDeque};
use blockchain::import::BlockImporter;
use futures::{Stream, StreamExt};
use futures_timer::Interval;
use log::*;
use rand::seq::IteratorRandom;

pub struct PeerStatus<H> {
	head_status: Option<(H, usize)>,
	pending_request: Option<usize>,
}

impl<H> Default for PeerStatus<H> {
	fn default() -> Self {
		Self {
			head_status: None,
			pending_request: None,
		}
	}
}

#[derive(PartialEq, Eq)]
pub enum SyncEvent<P> {
	QueryStatus,
	QueryPeerStatus(P),
	QueryBlocks(P),
}

#[derive(PartialEq, Eq)]
pub struct SyncConfig {
	pub peer_update_frequency: usize,
	pub update_frequency: usize,
	pub request_timeout: usize,
}

pub struct NetworkSync<P, H, I: BlockImporter> {
	head_status: (H, usize),
	tick: usize,
	peers: HashMap<P, PeerStatus<H>>,
	pending_blocks: Vec<I::Block>,
	importer: I,
	waker: Option<Waker>,
	timer: Interval,
	pending_events: VecDeque<SyncEvent<P>>,
	last_sync: Option<usize>,
	config: SyncConfig,
}

impl<P, H, I> NetworkSync<P, H, I> where
	I: BlockImporter,
	P: PartialEq + Eq + Hash,
	H: PartialOrd,
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
			last_sync: None,
			config,
		}
	}

	pub fn note_blocks(&mut self, mut blocks: Vec<I::Block>, _source: Option<P>) {
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

impl<P, H, I> Stream for NetworkSync<P, H, I> where
	P: PartialEq + Eq + Hash + Clone + Unpin,
	H: PartialOrd + Unpin,
	I: BlockImporter + Unpin,
	I::Block: Clone + Unpin,
	I::Error: core::fmt::Debug,
{
	type Item = SyncEvent<P>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		self.waker = Some(cx.waker().clone());

		let mut pending_blocks = Vec::new();
		mem::swap(&mut self.pending_blocks, &mut pending_blocks);
		let mut pending_blocks = pending_blocks.into_iter().map(|v| Some(v)).collect::<Vec<_>>();

		loop {
			let mut progress = false;

			for block in &mut pending_blocks {
				if let Some(sblock) = block {
					match self.importer.import_block(sblock.clone()) {
						Ok(()) => {
							*block = None;
							progress = true;
							trace!("Imported one block");
						},
						Err(e) => {
							warn!("Import block failed: {:?}", e);
						},
					}
				}
			}

			if !progress {
				break
			}
		}

		let unimported_blocks = pending_blocks.iter().filter(|v| v.is_some()).count();
		if unimported_blocks != 0 {
			warn!("{} blocks cannot be imported", unimported_blocks);
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
		let current_tick = self.tick;
		let request_timeout = self.config.request_timeout;
		let update_frequency = self.config.update_frequency;
		let peer_update_frequency = self.config.peer_update_frequency;

		if self.tick - self.head_status.1 >= self.config.update_frequency {
			new_events.push(SyncEvent::QueryStatus);
		}

		for (peer, status) in &mut self.peers {
			if let Some(last_tick) = status.pending_request {
				if current_tick - last_tick >= request_timeout {
					status.pending_request = None;
				}
			}

			if status.pending_request.is_none() && status.head_status.as_ref()
				.map(|h| current_tick - h.1 >= peer_update_frequency)
				.unwrap_or(true)
			{
				new_events.push(SyncEvent::QueryPeerStatus(peer.clone()));
				status.pending_request = Some(current_tick);
			}
		}

		if self.is_syncing() {
			let mut need_initialize_new_request = false;

			for (_, status) in &self.peers {
				if status.head_status.as_ref().map(|h| h.0 > self.head_status.0).unwrap_or(false) {
					need_initialize_new_request = true;
				}
			}

			need_initialize_new_request = need_initialize_new_request && self.last_sync.map(|l| {
				current_tick - l >= update_frequency
			}).unwrap_or(true);

			if need_initialize_new_request {
				if let Some((peer, status)) = self.peers.iter_mut().choose(&mut rand::thread_rng()) {
					new_events.push(SyncEvent::QueryBlocks(peer.clone()));
					status.pending_request = Some(current_tick);
				}

				self.last_sync = Some(self.tick);
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
