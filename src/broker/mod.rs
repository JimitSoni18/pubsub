use std::{
	borrow::Cow,
	default,
	hash::Hash,
	sync::atomic::{AtomicU64, Ordering},
};

use crate::Message;

use dashmap::{DashMap, DashSet};
use tokio::sync::mpsc::{self, Sender};

// TODO: use Arc<str> for channel name
#[derive(Default)]
pub struct Broker<'a, M> {
	// channel name -> client ids
	// TODO: use Arc<str> or something lighter for channel
	channels: DashMap<Cow<'a, str>, DashSet<ClientId>>,
	// names?
	subscribers: DashMap<ClientId, mpsc::Sender<M>>,
	seq: AtomicU64,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct ClientId(u64);

// as ClientId should not be created form u64, the broker depends on the guarantee that the client
// id will always have a corresponding channel
#[allow(clippy::from_over_into)]
impl Into<u64> for ClientId {
	fn into(self) -> u64 {
		self.0
	}
}

pub struct Client<M> {
	id: ClientId,
	rx: mpsc::Receiver<M>,
}

impl<M> Client<M> {
	pub fn id(&self) -> ClientId {
		self.id
	}

	pub fn receiver(&mut self) -> &mut mpsc::Receiver<M> {
		&mut self.rx
	}
}

pub const MESSAGE_CHANNEL_QUEUE_LIMIT: usize = 8;

impl<'a, M> Broker<'a, M> {
	pub fn new_client(&self) -> (Sender<M>, Client<M>) {
		let (tx, rx) = mpsc::channel::<M>(MESSAGE_CHANNEL_QUEUE_LIMIT);
		let id = self.issue_id();
		self.subscribers.insert(id, tx.clone());

		(tx, Client::<M> { id, rx })
	}

	pub fn subscribe(&self, channel: String, client_id: ClientId) -> Result<(), SubscribeError> {
		if self.subscribers.get(&client_id).is_none() {
			// this should never be true
			return Err(SubscribeError::ClientNotRegistered);
		}

		match self.channels.entry(Cow::Owned(channel)) {
			dashmap::Entry::Occupied(occupied_entry) => {
				occupied_entry.get().insert(client_id);
			}
			_ => return Err(SubscribeError::ChannelNotFound),
		}

		Ok(())
	}

	/// unsubscribe from a particular channel
	pub fn unsubscribe(&self, channel: &str, client_id: ClientId) -> Result<(), UnsubscribeError> {
		self.channels
			.get(&*Cow::Borrowed(channel))
			.ok_or(UnsubscribeError::ChannelNotFound)?
			.remove(&client_id)
			.ok_or(UnsubscribeError::AlreadyNotSubscribed)?;

		Ok(())
	}

	// TODO: return count/list?
	/// unsubscibe from all channels
	pub fn unsubscribe_all(&self, client_id: ClientId) {
		self.channels.iter().for_each(|set| {
			set.remove(&client_id);
		});
	}

	pub(super) fn issue_id(&self) -> ClientId {
		ClientId(self.seq.fetch_add(1, Ordering::Relaxed))
	}

	pub async fn publish(&self, channel_name: &str, message: M) -> Result<usize, PublishError>
	where
		M: Clone,
	{
		let subscriber_ids = self
			.channels
			.get(&*Cow::Borrowed(channel_name))
			.ok_or(PublishError::ChannelNotFound)?;

		let count = futures::future::join_all(subscriber_ids.iter().filter_map(|id| {
			self.subscribers.get(&id).map(|sender| {
				let msg_clone = message.clone();
				async move {
					// if it errors out, it means receiver is dropped
					let rx_dropped = sender.send(msg_clone).await.is_err();
					if rx_dropped {
						self.subscribers.remove(&id);
						self.channels.iter().for_each(|c| {
							c.remove(&id);
						});
					}
					rx_dropped
				}
			})
		}))
		.await
		.into_iter()
		.filter(|b| !b)
		.count();

		Ok(count)
	}

	// maybe just return bool?: true -> channel created; false -> channel already exists
	// and then:
	// #[must_use]
	pub fn create_channel(&self, channel_name: &str) -> Result<(), NewChannelError> {
		let channel_name = Cow::Borrowed(channel_name);
		if self.channels.contains_key(&*channel_name) {
			return Err(NewChannelError::ChannelAlreadyExists);
		}

		self.channels
			.insert(Cow::Owned(channel_name.into_owned()), Default::default());

		Ok(())
	}

	// TODO: unregister/destroy client?; 48 hours later: not planned
}

#[derive(Debug)]
pub enum SubscribeError {
	ChannelNotFound,
	ClientNotRegistered,
}

#[derive(Debug)]
pub enum NewChannelError {
	ChannelAlreadyExists,
}

#[derive(Debug)]
pub enum UnsubscribeError {
	ChannelNotFound,
	AlreadyNotSubscribed,
	SubscriberMissing,
}

#[derive(Debug)]
pub enum PublishError {
	ChannelNotFound,
}
