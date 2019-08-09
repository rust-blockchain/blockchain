pub mod sync;

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
